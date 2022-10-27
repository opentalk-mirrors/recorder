#[derive(Debug)]
pub enum Error {
    TooManyParticipants,
    TooManyVisibles,
    ParticipantNotFound(String),
    PlayingPipelineForbidden,
    MoreMaxVisiblesThanMaxParticipants,
    IdDoublet(String),
}
