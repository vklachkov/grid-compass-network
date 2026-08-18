use std::{
    sync::Arc,
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use log::{debug, error, info, warn};
use minijinja::{Environment, context};
use tiny_http::{Header, Request, Response, Server};

use crate::{db, shared};

const POLL_INTERVAL_MS: u32 = 5000;

/// Templates are compiled in so a missing file fails the build rather than
/// every page at runtime.
fn environment() -> Result<Environment<'static>> {
    let mut env = Environment::new();

    // Otherwise every control tag leaves its own blank line in the output, and
    // the rendered page reads nothing like the templates it came from.
    env.set_trim_blocks(true);
    env.set_lstrip_blocks(true);

    env.add_template("layout.html", include_str!("templates/layout.html"))?;
    env.add_template(
        "users_table.html",
        include_str!("templates/users_table.html"),
    )?;
    env.add_template("users.html", include_str!("templates/users.html"))?;

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

fn handle(request: Request, env: &Environment<'_>, db_path: &str) -> Result<()> {
    let url = request.url().to_owned();
    let path = url.split('?').next().unwrap_or("");

    match path {
        "/" => respond(request, redirect("/users")),
        "/users" => respond(request, page(env, db_path, "users.html")),
        "/users/table" => respond(request, page(env, db_path, "users_table.html")),
        "/GRiD.png" => request.respond(logo()).context("write the response"),
        _ => respond(request, not_found()),
    }
}

fn logo() -> Response<std::io::Cursor<Vec<u8>>> {
    const LOGO: &[u8] = include_bytes!("static/GRiD.png");

    let content_type = Header::from_bytes(&b"Content-Type"[..], &b"image/png"[..])
        .expect("a constant header should parse");

    Response::from_data(LOGO).with_header(content_type)
}

fn page(
    env: &Environment<'_>,
    db_path: &str,
    template: &str,
) -> Response<std::io::Cursor<Vec<u8>>> {
    match render(env, db_path, template) {
        Ok(html) => html_response(200, html),
        Err(err) => {
            warn!(target: "web", "failed to render {template}: {err}");
            html_response(500, "<H1>500</H1><P>The page could not be rendered.".into())
        }
    }
}

fn render(env: &Environment<'_>, db_path: &str, template: &str) -> Result<String> {
    // A connection per request instead of one behind a lock: WAL lets these
    // reads run alongside the session threads' writes.
    let conn = db::open(db_path)?;
    let accounts = db::load_by_age(&conn).context("read the account directory")?;

    let rows: Vec<_> = accounts.iter().map(row).collect();

    Ok(env.get_template(template)?.render(context! {
        title => "Users",
        poll_interval_ms => POLL_INTERVAL_MS,
        refreshed_at => clock_time(),
        accounts => rows,
    })?)
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
