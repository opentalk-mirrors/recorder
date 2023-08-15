use glib::Cast;
use gst::{traits::GstObjectExt, DebugGraphDetails};

/// Pipeline DOT debugging parameters
pub struct Params {
    /// Graphics details like described in gstreamer
    pub details: DebugGraphDetails,
    /// Use an index prefix for the output files
    pub index: bool,
}

impl Params {
    pub const fn all() -> Self {
        Self {
            details: DebugGraphDetails::ALL,
            index: true,
        }
    }
    pub const fn states() -> Self {
        Self {
            details: DebugGraphDetails::STATES,
            index: true,
        }
    }
}

impl Default for Params {
    fn default() -> Self {
        Self {
            details: DebugGraphDetails::ALL,
            index: true,
        }
    }
}

/// make a DOT file of the given element if log level is debug
pub fn debug_dot(bin: &impl glib::IsA<gst::Element>, filename_without_extension: &str) {
    if log::max_level() >= log::Level::Debug {
        dot(bin, filename_without_extension);
    }
}

/// make a DOT file of the given element with a counting index and default parameters
pub fn dot(bin: &impl glib::IsA<gst::Element>, filename_without_extension: &str) {
    dot_ext(bin, filename_without_extension, &Default::default());
}

/// make a DOT file of the given element with a counting index and the given parameters
pub fn dot_ext(
    bin: &impl glib::IsA<gst::Element>,
    filename_without_extension: &str,
    params: &Params,
) {
    // count calls
    use std::sync::atomic::{AtomicUsize, Ordering};
    static COUNT: AtomicUsize = AtomicUsize::new(0);

    // check if env var `GST_DEBUG_DUMP_DOT_DIR` has been set properly
    let Ok(path) = std::env::var("GST_DEBUG_DUMP_DOT_DIR") else {
        if COUNT.load(Ordering::SeqCst) == 0 {
            debug!("You need to set GST_DEBUG_DUMP_DOT_DIR in environment to an absolute path to get DOT output.");
        };
        return;
    };

    if let Err(e) = std::fs::create_dir_all(path.clone()) {
        error!("can not create dir from GST_DEBUG_DUMP_DOT_DIR: {:?}", e);
        return;
    };

    // recursion to top parent
    if let Some(parent) = bin.clone().dynamic_cast::<gst::Object>().unwrap().parent() {
        return dot(
            &parent.dynamic_cast::<gst::Element>().unwrap(),
            filename_without_extension,
        );
    }

    let name = if params.index {
        let n = COUNT.fetch_add(1, Ordering::SeqCst);
        let r = format!("{n}-{filename_without_extension}");
        r
    } else {
        filename_without_extension.to_string()
    };

    info!("GENERATING DOT FILE: '{path}/{name}.dot'");

    gst::debug_bin_to_dot_file(
        &Cast::dynamic_cast::<gst::Bin>(bin.clone()).unwrap(),
        params.details,
        name,
    );
}

/// create a name (and parent name as suffix) from given gstreamer object
pub fn name(object: &impl glib::IsA<gst::Object>) -> glib::GString {
    if let Some(parent) = object.parent() {
        format!("{}.{}", name(&parent), object.name()).into()
    } else {
        object.name()
    }
}
