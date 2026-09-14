mod broadcast;
mod postoffice;
mod protocol;

#[cfg(test)]
mod tests;

pub use broadcast::MailBroadcastServer;
pub use postoffice::MailServer;
