use glib::Cast;
use gst::{traits::GstObjectExt, DebugGraphDetails};

pub struct Params {
    pub details: DebugGraphDetails,
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

pub fn dot(bin: &impl glib::IsA<gst::Element>, filename_without_extension: &str) {
    dot_ext(bin, filename_without_extension, &Default::default());
}

pub fn dot_ext(
    bin: &impl glib::IsA<gst::Element>,
    filename_without_extension: &str,
    params: &Params,
) {
    // count calls
    use std::sync::atomic::{AtomicUsize, Ordering};
    static COUNT: AtomicUsize = AtomicUsize::new(0);

    // check if env var 'GST_DEBUG_DUMP_DOT_DIR' has been set properly
    if let Ok(path) = std::env::var("GST_DEBUG_DUMP_DOT_DIR") {
        // find the parent
        match bin.clone().dynamic_cast::<gst::Object>().unwrap().parent() {
            Some(parent) => dot(
                &parent.dynamic_cast::<gst::Element>().unwrap(),
                filename_without_extension,
            ),
            None => {
                let name = if params.index {
                    let n = COUNT.fetch_add(1, Ordering::SeqCst);
                    let r = format!("{n}-{filename_without_extension}");
                    r
                } else {
                    format!("{filename_without_extension}")
                };

                info!("GENERATING DOT FILE: '{path}/{name}.dot'");

                gst::debug_bin_to_dot_file(
                    &glib::Cast::dynamic_cast::<gst::Bin>(bin.clone()).unwrap(),
                    params.details,
                    name,
                );
            }
        }
    } else if COUNT.load(Ordering::SeqCst) == 0 {
        warn!("Can not write DOT file. You need to set GST_DEBUG_DUMP_DOT_DIR in environment to an absolute path to get DOT output.");
        COUNT.fetch_add(1, Ordering::SeqCst);
    }
}

pub fn name(object: &impl glib::IsA<gst::Object>) -> glib::GString {
    if let Some(parent) = object.parent() {
        format!("{}.{}", name(&parent), object.name()).into()
    } else {
        object.name()
    }
}
