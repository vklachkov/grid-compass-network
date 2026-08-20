use std::rc::Rc;

use super::{broadcast::*, postoffice::*, protocol::*};
use crate::{db, db::mailbox, db::mailbox::Message};

const STORED_BODY: &str = "Stored body";

fn mail_server() -> MailServer {
    let conn = Rc::new(db::open_in_memory());
    let owner = db::find_user(&conn, "GRiD", "Demo", "GUEST")
        .expect("read the demo directory")
        .expect("GUEST should exist");

    MailServer::new(conn, owner.id, owner.user)
}

/// Every test that reads a mailbox has to fill it first: a fresh database
/// holds no mail at all.
fn with_one_message() -> MailServer {
    let mut mail = mail_server();
    assert!(mail.accept_outgoing(outgoing("Stored subject", STORED_BODY)));
    mail
}

/// GUEST addresses itself: a recipient has to be an account of the sender's
/// own group, and the mailbox the tests then read is the sender's.
fn outgoing(subject: &str, body: &str) -> Vec<u8> {
    let mut data = app_frame(RECORD_MARKER, b"tGUEST");
    data.extend(tagged_record(b's', subject));
    data.extend(tagged_record(b'n', body));
    data.extend(app_frame(RECORD_MARKER, &[TAG_TERMINATOR]));
    data
}

fn stored_message(mail: &MailServer, mail_id: u32) -> Message {
    mailbox::find(&mail.conn, mail.owner_id, mail_id)
        .expect("read the mailbox")
        .expect("the message should be stored")
}

fn detail_query(mail_id: u32) -> Vec<u8> {
    let mut query = vec![0xfd, 27, 0, b'S', 7, 0, 0];
    query.extend(MailId::from_u32(mail_id).wire_bytes());
    query.extend(b"kuvspoftcandgbzxy");
    query
}

fn request(more: bool, data: &[u8]) -> Vec<u8> {
    let mut payload = vec![more as u8, 5, 0, 0];
    payload.extend_from_slice(data);
    payload
}

fn response(note: u16, marker: u8, data: &[u8]) -> Vec<MailResponse> {
    vec![MailResponse {
        note,
        payload: transport(0, 5, &app_frame(marker, data)),
    }]
}

#[test]
fn initializes_session_from_observed_payload() {
    let mut mail = mail_server();
    let initialize = SessionInitialize {
        service_id: MAIL_SERVICE_ID,
        protocol_version: PROTOCOL_VERSION,
    }
    .encode();
    let mut request = vec![0, 5, 0, 0];
    request.extend(initialize);
    assert_eq!(mail.process(0, &request), Some(response(0, 0xfe, b"z")));
}

#[test]
fn assembles_transport_fragments() {
    let mut mail = mail_server();
    assert_eq!(
        mail.process(6, &request(true, &[0xfd, 2, 0, b't', b'x'])),
        None
    );
    assert_eq!(
        mail.process(6, &request(false, &[])),
        Some(response(6, 0xfd, b"T"))
    );
}

#[test]
fn drops_interleaved_connection() {
    let mut mail = mail_server();
    assert_eq!(
        mail.process(6, &request(true, &[0xfd, 2, 0, b't', b'x'])),
        None
    );
    assert_eq!(mail.process(6, &[0, 6, 0, 0]), None);
}

#[test]
fn rejects_malformed_commands() {
    let mut mail = mail_server();
    assert_eq!(mail.process(7, &request(false, &[])), None);
    assert_eq!(mail.process(8, &request(false, b"x")), None);
    assert_eq!(
        mail.process(0x0d, &request(false, &[0xfd, 3, 0, b'I'])),
        None
    );
}

#[test]
fn completes_initial_server_message_drain() {
    let mut mail = mail_server();
    assert_eq!(
        mail.process(0x10, &[0, 1, 0, 0]),
        Some(vec![MailResponse {
            note: 0x10,
            payload: transport(0, 1, &app_frame(RECORD_MARKER, &[TAG_TERMINATOR])),
        }])
    );
}

#[test]
fn an_empty_mailbox_lists_nothing() {
    let mut mail = mail_server();
    let query = [0xfd, 10, 0, b'S', 7, 0, 1, 0, 0, 0, 0, 0, 0];
    assert_eq!(
        mail.process(7, &request(false, &query)),
        Some(vec![MailResponse {
            note: 7,
            payload: transport(0, 5, &empty_mail_result()),
        }])
    );
}

#[test]
fn returns_the_stored_mail_list() {
    let mut mail = with_one_message();
    let query = [0xfd, 10, 0, b'S', 7, 0, 1, 0, 0, 0, 0, 0, 0];
    let [response] = mailbox_list(&[stored_message(&mail, 1)])
        .try_into()
        .unwrap();
    assert_eq!(
        mail.process(7, &request(false, &query)),
        Some(vec![MailResponse {
            note: 7,
            payload: transport(response.flags, 5, &response.data),
        }])
    );
}

#[test]
fn returns_complete_mail_for_detail_query() {
    let mut mail = with_one_message();
    let expected = mail_detail(&stored_message(&mail, 1));

    let responses = mail.process(7, &request(false, &detail_query(1))).unwrap();
    let [response] = responses.as_slice() else {
        panic!("detail query returned more than one response");
    };
    assert_eq!(response.note, 7);
    assert_eq!(response.payload, transport(0, 5, &expected));

    let tagged_end = response
        .payload
        .windows(4)
        .position(|bytes| bytes == [0xfd, 1, 0, b'z'])
        .unwrap()
        + 4;
    assert_eq!(&response.payload[tagged_end..], STORED_BODY.as_bytes());
    assert_eq!(response.payload[0] & MORE, 0);
}

/// The upper two bytes of a mail id are always zero, so a query that sets
/// them addresses no message the server could have handed out.
#[test]
fn a_detail_query_for_an_unknown_id_returns_nothing() {
    let mut mail = with_one_message();
    let mut query = vec![0xfd, 27, 0, b'S', 7, 0, 0];
    query.extend([1, 0, 0, 0, 1, 0]);
    query.extend(b"kuvspoftcandgbzxy");

    assert_eq!(
        mail.process(7, &request(false, &query)),
        Some(vec![MailResponse {
            note: 7,
            payload: transport(0, 5, &empty_mail_result()),
        }])
    );
}

fn read_new_query() -> Vec<u8> {
    let mut query = vec![0xfd, 27, 0, b'S', 1, 0, 1];
    query.extend([0; 6]);
    query.extend(b"kuvspoftcandgbzxy");
    query
}

#[test]
fn returns_stored_mail_once_for_read_new_query() {
    let mut mail = with_one_message();
    let query = read_new_query();
    let expected = mail_detail(&stored_message(&mail, 1));

    assert_eq!(
        mail.process(7, &request(false, &query)),
        Some(vec![MailResponse {
            note: 7,
            payload: transport(0, 5, &expected),
        }])
    );
    assert_eq!(
        mail.process(7, &request(false, &query)),
        Some(vec![MailResponse {
            note: 7,
            payload: transport(0, 5, &empty_mail_result()),
        }])
    );
}

#[test]
fn read_new_state_does_not_hide_mail_from_normal_detail_query() {
    let mut mail = with_one_message();
    let expected = mail_detail(&stored_message(&mail, 1));
    mail.process(7, &request(false, &read_new_query())).unwrap();

    assert_eq!(
        mail.process(7, &request(false, &detail_query(1))),
        Some(vec![MailResponse {
            note: 7,
            payload: transport(0, 5, &expected),
        }])
    );
}

#[test]
fn returns_each_mailbox_item_in_its_own_response() {
    let mut mail = with_one_message();
    assert!(mail.accept_outgoing(outgoing("Second", "Second body")));

    let query = [0xfd, 10, 0, b'S', 7, 0, 1, 0, 0, 0, 0, 0, 0];
    let responses = mail.process(7, &request(false, &query)).unwrap();
    assert_eq!(responses.len(), 2);
    assert_eq!(responses[0].payload[0] & MORE, MORE);
    assert_eq!(responses[1].payload[0] & MORE, 0);

    for response in &responses {
        let mut offset = 4;
        let mut tags = Vec::new();
        while offset < response.payload.len() {
            assert_eq!(response.payload[offset], 0xfd);
            let length =
                u16::from_le_bytes([response.payload[offset + 1], response.payload[offset + 2]])
                    as usize;
            tags.push(response.payload[offset + 3]);
            offset += 3 + length;
        }
        assert_eq!(tags, [b'b', b'k', b's', b'z']);
    }

    assert_ne!(&responses[0].payload[8..14], &responses[1].payload[8..14]);
}

#[test]
fn accepts_outgoing_mail_and_selects_it_by_id() {
    let mut data = app_frame(RECORD_MARKER, b"tGUEST");
    data.extend(app_frame(RECORD_MARKER, b"sSent subject"));
    data.extend(app_frame(RECORD_MARKER, b"DDemo attachment nnoise"));
    data.extend(app_frame(RECORD_MARKER, b"nSent body"));
    data.extend(app_frame(RECORD_MARKER, &[TAG_TERMINATOR]));

    let mut mail = with_one_message();
    assert!(mail.accept_outgoing(data));

    let stored = stored_message(&mail, 2);
    assert_eq!(stored.subject, "Sent subject");
    assert_eq!(stored.body, "Sent body");

    let responses = mail.process(7, &request(false, &detail_query(2))).unwrap();
    let [response] = responses.as_slice() else {
        panic!("detail query returned more than one response");
    };
    assert!(
        response
            .payload
            .windows(b"Sent subject".len())
            .any(|bytes| bytes == b"Sent subject")
    );
    assert!(response.payload.ends_with(b"Sent body"));
}

#[test]
fn accepts_outgoing_mail_with_a_stale_tail() {
    let mut data = outgoing("Short", "Short body");
    data.extend_from_slice(b" leftovers of a longer object");

    let mut mail = mail_server();
    assert!(mail.accept_outgoing(data));

    let stored = stored_message(&mail, 1);
    assert_eq!(stored.subject, "Short");
    assert_eq!(stored.body, "Short body");
}

#[test]
fn read_new_iterates_all_mail_then_terminates() {
    let mut mail = with_one_message();
    assert!(mail.accept_outgoing(outgoing("Second", "Second body")));

    let query = read_new_query();
    let first = mail.process(7, &request(false, &query)).unwrap();
    let second = mail.process(7, &request(false, &query)).unwrap();
    let third = mail.process(7, &request(false, &query)).unwrap();
    assert!(first[0].payload.ends_with(STORED_BODY.as_bytes()));
    assert!(second[0].payload.ends_with(b"Second body"));
    assert_eq!(third[0].payload, transport(0, 5, &empty_mail_result()));
}

#[test]
fn rejects_mail_that_cannot_fit_in_one_vipc_response() {
    let mut mail = mail_server();

    assert!(!mail.accept_outgoing(outgoing("Large", &"x".repeat(40_000))));
    assert!(mailbox::list(&mail.conn, mail.owner_id).unwrap().is_empty());
}

/// The mailbox belongs to the account that signed on, so a message stored
/// by one session is invisible to a session signed on as somebody else.
#[test]
fn a_mailbox_is_not_shared_between_accounts() {
    let mail = with_one_message();
    let other = db::find_user(&mail.conn, "GRiD", "Systems", "MANAGER")
        .unwrap()
        .unwrap();
    let mut other_mail = MailServer::new(mail.conn.clone(), other.id, other.user);

    assert!(
        mailbox::list(&other_mail.conn, other.id)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        other_mail.process(7, &request(false, &detail_query(1))),
        Some(vec![MailResponse {
            note: 7,
            payload: transport(0, 5, &empty_mail_result()),
        }])
    );
}

#[test]
fn rejects_transport_error() {
    assert_eq!(mail_server().process(0, &[0, 5, 1, 0]), None);
}

fn broadcast_request(flags: u8, data: &[u8]) -> Vec<u8> {
    let mut payload = vec![flags, 1, 0, 0];
    payload.extend_from_slice(data);
    payload
}

#[test]
fn answers_initialization() {
    let mut server = MailBroadcastServer::new();

    let initialize = SessionInitialize {
        service_id: BROADCAST_SERVICE_ID,
        protocol_version: PROTOCOL_VERSION,
    }
    .encode();
    let response = server.process(&broadcast_request(0, &initialize));

    assert_eq!(response, Some(vec![0, 1, 0, 0, 0xfe, 1, 0, b'z']));
}

#[test]
fn keeps_the_connection_id_of_the_request() {
    let mut server = MailBroadcastServer::new();
    let mut payload = vec![0, 7, 0, 0];
    payload.extend(
        SessionInitialize {
            service_id: BROADCAST_SERVICE_ID,
            protocol_version: PROTOCOL_VERSION,
        }
        .encode(),
    );

    let response = server.process(&payload).unwrap();

    assert_eq!(response[1], 7);
}

#[test]
fn ignores_continuation_fragments() {
    let mut server = MailBroadcastServer::new();

    let initialize = SessionInitialize {
        service_id: BROADCAST_SERVICE_ID,
        protocol_version: PROTOCOL_VERSION,
    }
    .encode();
    assert_eq!(server.process(&broadcast_request(MORE, &initialize)), None);
}

#[test]
fn ignores_frames_reporting_an_error() {
    let mut server = MailBroadcastServer::new();
    let mut payload = vec![0, 1, 5, 0];
    payload.extend(
        SessionInitialize {
            service_id: BROADCAST_SERVICE_ID,
            protocol_version: PROTOCOL_VERSION,
        }
        .encode(),
    );

    assert_eq!(server.process(&payload), None);
}

#[test]
fn ignores_unknown_frames() {
    let mut server = MailBroadcastServer::new();

    assert_eq!(
        server.process(&broadcast_request(0, &[0xfd, 1, 0, b'z'])),
        None
    );
    assert_eq!(server.process(&broadcast_request(0, &[])), None);
    assert_eq!(server.process(&[]), None);
}
