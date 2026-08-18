use std::{
    sync::Arc,
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use log::{debug, error, info, warn};
use minijinja::{Environment, context};
use serde::{Deserialize, Serialize};
use tiny_http::{Header, Request, Response, Server};

use crate::{db, db::mailbox, services::sentry::Authority, shared};

/// Templates are compiled in so a missing file fails the build rather than
/// every page at runtime.
fn environment() -> Result<Environment<'static>> {
    let mut env = Environment::new();

    // Otherwise every control tag leaves its own blank line in the output, and
    // the rendered page reads nothing like the templates it came from.
    env.set_trim_blocks(true);
    env.set_lstrip_blocks(true);

    env.add_template("layout.html", include_str!("templates/layout.html"))?;
    env.add_template("users.html", include_str!("templates/users.html"))?;
    env.add_template("new_user.html", include_str!("templates/new_user.html"))?;
    env.add_template("mailbox.html", include_str!("templates/mailbox.html"))?;
    env.add_template("message.html", include_str!("templates/message.html"))?;

    Ok(env)
}

pub fn serve() -> Result<()> {
    let addr = shared::env::read_env("WEB_LISTEN_ADDR")?;
    let db_path = shared::env::read_env("DB_PATH")?;

    let env = Arc::new(environment().context("compile the frontend templates")?);

    let server = Server::http(&addr)
        .map_err(|err| anyhow::anyhow!("{err}"))
        .with_context(|| format!("bind the frontend to {addr}"))?;

    info!(target: "web", "start frontend at http://{addr}/users");

    for request in server.incoming_requests() {
        let env = Arc::clone(&env);
        let db_path = db_path.clone();

        thread::spawn(move || {
            let addr = request
                .remote_addr()
                .map_or_else(|| "?".to_owned(), |addr| addr.to_string());

            debug!(target: "web", "{addr}: {} {}", request.method(), request.url());

            if let Err(err) = handle(request, &env, &db_path) {
                error!(target: "web", "{addr}: failed to answer: {err}");
            }
        });
    }

    Ok(())
}

fn handle(mut request: Request, env: &Environment<'_>, db_path: &str) -> Result<()> {
    let url = request.url().to_owned();
    let mut parts = url.splitn(2, '?');
    let path = parts.next().unwrap_or("");
    let raw_query = parts.next().unwrap_or("");
    let posted = request.method() == &tiny_http::Method::Post;

    match path {
        "/" => respond(request, redirect("/users")),
        "/users" => respond(request, page(env, db_path, "users.html", users)),
        "/users/new" if posted => {
            let form = read_body(&mut request);
            respond(
                request,
                page(env, db_path, "new_user.html", |conn| {
                    create_user(conn, &query(&form))
                }),
            )
        }
        "/users/new" => respond(
            request,
            page(env, db_path, "new_user.html", |_| Ok(new_user_form())),
        ),
        "/mailbox" => respond(
            request,
            page(env, db_path, "mailbox.html", |conn| {
                mailbox(conn, &query(raw_query))
            }),
        ),
        "/mailbox/message" => respond(
            request,
            page(env, db_path, "message.html", |conn| {
                message(conn, &query(raw_query))
            }),
        ),
        "/GRiD.png" => request.respond(logo()).context("write the response"),
        _ => respond(request, not_found()),
    }
}

fn logo() -> Response<std::io::Cursor<Vec<u8>>> {
    const LOGO: &[u8] = include_bytes!("static/GRiD.png");

    let content_type = Header::from_bytes(&b"Content-Type"[..], &b"image/png"[..])
        .expect("a constant header should parse");

    // Long enough that the logo is not refetched on every page, short enough
    // that replacing it does not leave the old one cached.
    let cache = Header::from_bytes(&b"Cache-Control"[..], &b"public, max-age=3600"[..])
        .expect("a constant header should parse");

    Response::from_data(LOGO)
        .with_header(content_type)
        .with_header(cache)
}

fn page(
    env: &Environment<'_>,
    db_path: &str,
    template: &str,
    build: impl FnOnce(&rusqlite::Connection) -> Result<minijinja::Value>,
) -> Response<std::io::Cursor<Vec<u8>>> {
    match render(env, db_path, template, build) {
        Ok(html) => html_response(200, html),
        Err(err) => {
            warn!(target: "web", "failed to render {template}: {err}");
            html_response(500, "<H1>500</H1><P>The page could not be rendered.".into())
        }
    }
}

fn render(
    env: &Environment<'_>,
    db_path: &str,
    template: &str,
    build: impl FnOnce(&rusqlite::Connection) -> Result<minijinja::Value>,
) -> Result<String> {
    // A connection per request instead of one behind a lock: WAL lets these
    // reads run alongside the session threads' writes.
    let conn = db::open(db_path)?;

    Ok(env.get_template(template)?.render(build(&conn)?)?)
}

fn users(conn: &rusqlite::Connection) -> Result<minijinja::Value> {
    let accounts = db::load_by_age(conn).context("read the account directory")?;
    let rows: Vec<_> = accounts.iter().map(row).collect();

    Ok(context! {
        title => "Users",
        nav => "users",
        refreshed_at => clock_time(),
        accounts => rows,
    })
}

fn row(account: &db::Account) -> minijinja::Value {
    let level = match account.level {
        db::LEVEL_COMPANY => "company",
        db::LEVEL_USER => "user",
        _ => "group",
    };

    // Companies and groups carry an authority too, but it is the level's own
    // constant rather than a property of the row, so printing it only invites
    // the reader to look for a difference that is not there.
    let authority = if account.level == db::LEVEL_USER {
        crate::services::sentry::Authority::from_stored(account.authority).name()
    } else {
        ""
    };

    context! {
        level => level,
        company => &account.company,
        group => &account.group,
        user => &account.user,
        password => &account.password,
        authority => authority,
        quota => quota(account.quota),
        used => account.used,
    }
}

#[derive(Default, Deserialize)]
struct NewUser {
    #[serde(default)]
    company: String,
    #[serde(default)]
    group: String,
    #[serde(default)]
    user: String,
    #[serde(default)]
    password: String,
    /// A browser that follows the form sends one of the offered levels, so an
    /// unparsable value is already a caller that went around the page — it is
    /// carried through as a number and refused by name below.
    #[serde(default)]
    authority: u16,
}

/// The quota the Sentry gives an account it is not told a size for. The page
/// asks for no quota of its own, so every user it creates starts unlimited and
/// is narrowed from the client if that is wanted.
const NEW_USER_QUOTA: u32 = u32::MAX;

fn new_user_form() -> minijinja::Value {
    form_context(&NewUser::default(), context! {})
}

fn form_context(form: &NewUser, extra: minijinja::Value) -> minijinja::Value {
    let authorities: Vec<_> = Authority::LEVELS
        .iter()
        .map(|level| context! { value => level.stored(), name => level.name() })
        .collect();

    context! {
        title => "New user",
        nav => "users",
        authorities => authorities,
        company => &form.company,
        group => &form.group,
        user => &form.user,
        password => &form.password,
        authority => form.authority,
        ..extra
    }
}

/// Every check the page makes is made here rather than in the browser: the form
/// is only a convenience, and the same POST can arrive without ever having been
/// drawn.
fn create_user(conn: &rusqlite::Connection, form: &NewUser) -> Result<minijinja::Value> {
    if let Some(error) = validate_new_user(conn, form)? {
        return Ok(form_context(form, context! { alert => error }));
    }

    let group_id = db::find_group(conn, &form.company, &form.group)
        .context("read the directory")?
        .context("the group should still exist")?;

    db::insert_user(
        conn,
        group_id,
        &form.user,
        &form.password,
        form.authority,
        NEW_USER_QUOTA,
    )
    .context("create the user")?;

    info!(target: "web", "created user {}/{}/{}", form.company, form.group, form.user);

    Ok(form_context(
        &NewUser::default(),
        context! {
            alert => format!(
                "Created the user {}/{}/{}.",
                form.company, form.group, form.user
            ),
        },
    ))
}

fn validate_new_user(conn: &rusqlite::Connection, form: &NewUser) -> Result<Option<String>> {
    if form.user.is_empty() {
        return Ok(Some("The user name is empty.".to_owned()));
    }

    if form.password.is_empty() {
        return Ok(Some("The password is empty.".to_owned()));
    }

    if !Authority::from_stored(form.authority).is_defined() {
        return Ok(Some(format!(
            "{} is not an authority level.",
            form.authority
        )));
    }

    if db::find_company(conn, &form.company)
        .context("read the directory")?
        .is_none()
    {
        return Ok(Some(format!("No such company: {}.", form.company)));
    }

    if db::find_group(conn, &form.company, &form.group)
        .context("read the directory")?
        .is_none()
    {
        return Ok(Some(format!("No such group: {}.", form.group)));
    }

    if db::find_user(conn, &form.company, &form.group, &form.user)
        .context("read the directory")?
        .is_some()
    {
        return Ok(Some(format!("The user {} already exists.", form.user)));
    }

    Ok(None)
}

/// The account whose mailbox is shown is named the way the Sentry names one, by
/// the whole company/group/user chain, because a bare user name is only unique
/// within its group.
fn mailbox(conn: &rusqlite::Connection, account_query: &AccountQuery) -> Result<minijinja::Value> {
    let AccountQuery {
        company,
        group,
        user,
    } = account_query;

    let base = context! {
        title => "Mailbox",
        nav => "mailbox",
        company => company,
        group => group,
        user => user,
    };

    if account_query.is_empty() {
        return Ok(base);
    }

    let Some(account) = db::find_user(conn, company, group, user).context("read the directory")?
    else {
        return Ok(context! { error => "No such user.", ..base });
    };

    let messages: Vec<_> = mailbox::list(conn, account.id)
        .context("read the mailbox")?
        .iter()
        .map(|message| {
            context! {
                mail_id => message.mail_id,
                sender => &message.sender,
                subject => &message.subject,
                attachment => message.attachment_path.as_deref().unwrap_or("None"),
                read => yes_no(message.is_read),
                href => account_query.message_href(message.mail_id),
            }
        })
        .collect();

    Ok(context! {
        account => format!("{company}/{group}/{user}"),
        messages => messages,
        ..base
    })
}

fn message(conn: &rusqlite::Connection, query: &MessageQuery) -> Result<minijinja::Value> {
    let MessageQuery { account, id } = query;

    let owner = db::find_user(conn, &account.company, &account.group, &account.user)
        .context("read the directory")?
        .context("the named user should exist")?;
    let message = mailbox::find(conn, owner.id, *id)
        .context("read the mailbox")?
        .context("the named message should exist")?;

    Ok(context! {
        title => "Message",
        nav => "mailbox",
        mailbox_href => account.mailbox_href(),
        message => context! {
            mail_id => message.mail_id,
            sender => message.sender,
            recipient => message.recipient,
            subject => message.subject,
            body => message.body,
            attachment => message.attachment_path,
            read => yes_no(message.is_read),
        },
    })
}

fn yes_no(value: bool) -> &'static str {
    if value { "Yes" } else { "No" }
}

/// The account a mailbox page is addressed by. A field the request omits reads
/// as empty rather than as an error: the page is reached with no query at all
/// before anything is searched for.
#[derive(Default, Deserialize, Serialize)]
struct AccountQuery {
    #[serde(default)]
    company: String,
    #[serde(default)]
    group: String,
    #[serde(default)]
    user: String,
}

impl AccountQuery {
    fn is_empty(&self) -> bool {
        self.company.is_empty() && self.group.is_empty() && self.user.is_empty()
    }

    fn mailbox_href(&self) -> String {
        format!(
            "/mailbox?{}",
            serde_urlencoded::to_string(self).unwrap_or_default()
        )
    }

    fn message_href(&self, mail_id: u32) -> String {
        format!(
            "/mailbox/message?{}&id={mail_id}",
            serde_urlencoded::to_string(self).unwrap_or_default()
        )
    }
}

#[derive(Default, Deserialize)]
struct MessageQuery {
    #[serde(flatten)]
    account: AccountQuery,
    #[serde(default)]
    id: u32,
}

/// A query that does not parse is answered as though it named nothing, which is
/// the empty search form rather than an error page.
fn query<T: Default + serde::de::DeserializeOwned>(query: &str) -> T {
    serde_urlencoded::from_str(query).unwrap_or_default()
}

/// A body that cannot be read is answered as an empty form, which the validation
/// below refuses by the same path as a form left blank.
fn read_body(request: &mut Request) -> String {
    let mut body = String::new();

    if let Err(err) = request.as_reader().read_to_string(&mut body) {
        warn!(target: "web", "failed to read a request body: {err}");
        body.clear();
    }

    body
}

/// The directory spells "no limit" as a saturated field rather than as a flag
/// of its own, and the raw number reads as a real quota.
fn quota(value: u32) -> String {
    if value == u32::MAX {
        "Unlimited".to_owned()
    } else {
        value.to_string()
    }
}

/// The stamp is the server's own wall clock: the page has to name a refresh
/// time in a browser whose date formatting cannot be relied on.
fn clock_time() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());

    let day = secs % 86_400;

    format!(
        "{:02}:{:02}:{:02} UTC",
        day / 3600,
        (day % 3600) / 60,
        day % 60
    )
}

fn html_response(status: u16, body: String) -> Response<std::io::Cursor<Vec<u8>>> {
    let header = Header::from_bytes(&b"Content-Type"[..], &b"text/html; charset=iso-8859-1"[..])
        .expect("a constant header should parse");

    // Without this IE serves the poll response from cache and the table freezes
    // on whatever it first drew.
    let cache = Header::from_bytes(&b"Cache-Control"[..], &b"no-cache"[..])
        .expect("a constant header should parse");

    Response::from_string(body)
        .with_status_code(status)
        .with_header(header)
        .with_header(cache)
}

fn redirect(target: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    let location =
        Header::from_bytes(&b"Location"[..], target.as_bytes()).expect("a route should parse");

    Response::from_string(String::new())
        .with_status_code(302)
        .with_header(location)
}

fn not_found() -> Response<std::io::Cursor<Vec<u8>>> {
    html_response(404, "<H1>404</H1><P>No such page.".into())
}

fn respond(request: Request, response: Response<std::io::Cursor<Vec<u8>>>) -> Result<()> {
    request.respond(response).context("write the response")
}
