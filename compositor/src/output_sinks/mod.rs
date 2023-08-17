mod dash_sink;
mod display_sink;
mod fake_sink;
mod matroska_sink;
mod mp4_sink;

pub use dash_sink::*;
pub use display_sink::*;
pub use fake_sink::*;
pub use matroska_sink::*;
pub use mp4_sink::*;

/// Universal sink that can be any sink.
#[derive(Debug)]
pub enum OutputSink {
    /// Acts as dash sink.
    Dash(DashSink),
    /// Acts as display sink.
    Display(DisplaySink),
    /// Acts as Matroska sink.
    Matroska(MatroskaSink),
    /// Acts as MP4 sink.
    Mp4(Mp4Sink),
}

/// Universal sink parameters that can be any sink parameters.
#[derive(Debug)]
pub enum OutputSinkParameters {
    /// Acts as dash sink parameters.
    Dash(DashParameters),
    /// Acts as display sink parameters.
    Display(DisplayParameters),
    /// Acts as Matroska sink parameters.
    Matroska(MatroskaParameters),
    /// Acts as MP4 sink parameters.
    Mp4(Mp4Parameters),
}

impl From<DashParameters> for OutputSinkParameters {
    fn from(params: DashParameters) -> Self {
        OutputSinkParameters::Dash(params)
    }
}

impl From<DisplayParameters> for OutputSinkParameters {
    fn from(params: DisplayParameters) -> Self {
        OutputSinkParameters::Display(params)
    }
}

impl From<MatroskaParameters> for OutputSinkParameters {
    fn from(params: MatroskaParameters) -> Self {
        OutputSinkParameters::Matroska(params)
    }
}

impl From<Mp4Parameters> for OutputSinkParameters {
    fn from(params: Mp4Parameters) -> Self {
        OutputSinkParameters::Mp4(params)
    }
}

impl crate::Sink for OutputSink {
    type Parameters = OutputSinkParameters;

    fn new(params: Self::Parameters) -> Self {
        match params {
            Self::Parameters::Dash(params) => OutputSink::Dash(DashSink::new(params)),
            Self::Parameters::Display(params) => OutputSink::Display(DisplaySink::new(params)),
            Self::Parameters::Matroska(params) => OutputSink::Matroska(MatroskaSink::new(params)),
            Self::Parameters::Mp4(params) => OutputSink::Mp4(Mp4Sink::new(params)),
        }
    }

    fn video(&self) -> gst::GhostPad {
        match self {
            Self::Dash(sink) => sink.video(),
            Self::Display(sink) => sink.video(),
            Self::Matroska(sink) => sink.video(),
            Self::Mp4(sink) => sink.video(),
        }
    }

    fn audio(&self) -> gst::GhostPad {
        match self {
            Self::Dash(sink) => sink.audio(),
            Self::Display(sink) => sink.audio(),
            Self::Matroska(sink) => sink.audio(),
            Self::Mp4(sink) => sink.audio(),
        }
    }

    fn bin(&self) -> gst::Bin {
        match self {
            Self::Dash(sink) => sink.bin(),
            Self::Display(sink) => sink.bin(),
            Self::Matroska(sink) => sink.bin(),
            Self::Mp4(sink) => sink.bin(),
        }
    }
}
