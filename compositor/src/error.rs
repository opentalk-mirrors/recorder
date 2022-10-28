#[derive(Debug)]
pub enum Error {
    /// Trying to add too many participants to the mixer
    /// (maximum value is defined by mixer configuration)
    TooManyParticipants,
    /// Trying to set too many participants in the mixer to be visible
    /// (maximum value is defined by mixer configuration)
    TooManyVisibles,
    /// Given participant can not be found within the mixer
    ParticipantNotFound(String),
    /// Called a method which needs to pause the pipeline before calling it
    PlayingPipelineForbidden,
    /// Failed to set more participants to visible as there are available in the mixer.
    MoreMaxVisiblesThanMaxParticipants,
    /// Failed to insert a new participant because there is already one in the mixer with the same ID.
    IdDoublet(String),
}
