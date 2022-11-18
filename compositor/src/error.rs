use core::fmt::Debug;

#[derive(Debug, thiserror::Error)]
pub enum Error<ID: Debug> {
    /// Trying to set too many participants in the mixer to be visible
    /// (maximum value is defined by mixer configuration)
    #[error("too many visible participants requested")]
    TooManyVisibles,
    /// Given participant can not be found within the mixer
    #[error("given participant id ({0:?}) cannot be found")]
    ParticipantNotFound(ID),
    /// Called a method which needs to pause the pipeline before calling it
    #[error("called function in playing pipeline")]
    PlayingPipelineForbidden,
    /// Failed to insert a new participant because there is already one in the mixer with the same ID.
    #[error("tried to insert already existing ID ({0:?})")]
    IdDoublet(ID),
}
