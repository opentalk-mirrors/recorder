#[derive(Debug)]
pub enum Error {
    TooManyParticipants,
    ParticipantNotFound(String),
    PlayingPipelineForbidden,
}
