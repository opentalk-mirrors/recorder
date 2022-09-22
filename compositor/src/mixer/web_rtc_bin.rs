use crate::mixer::mixer::*;
use gst::prelude::*;
use gst::Promise;
use gstreamer as gst;
use tokio::sync::oneshot;

pub struct WebRtcBin {
    pub bin: gst::Bin,
    pub audio_src: gst::GhostPad,
    pub video_src: gst::GhostPad,
    pub webrtcbin: gst::Element,
}

#[allow(dead_code)]
pub async fn create_web_rtc_bin(
    pipeline: &gst::Pipeline,
    name: &str,
    sdp_offer: &str,
) -> (WebRtcBin, String) {
    // prepare a bin with the dash recorder
    let bin = format!(
        r#"name={name}-webrtc-bin
    webrtcbin
        name=webrtc-{name}

    webrtc-{name}.
    ! rtpopusdepay
    ! opusdec
        name=audio-decoder
    ! fakesink
        name=audio-fakesink
    
    webrtc-{name}.
    ! rtpvp8depay
    ! vp8dec
        name=video-decoder
    ! fakesink
        name=video-fakesink
    "#,
    );

    // parse bin and add it to the pipeline
    info!("parsing test source bin `{name}`:\n{bin}");
    let bin = gst::parse_bin_from_description(&bin, false).unwrap();
    pipeline.add(&bin).unwrap();
    bin.set_state(gst::State::Playing).unwrap();

    // Start SDP negotiation
    let message = gst_sdp::SDPMessage::parse_buffer(sdp_offer.as_bytes()).unwrap();
    let description =
        gst_webrtc::WebRTCSessionDescription::new(gst_webrtc::WebRTCSDPType::Offer, message);

    let webrtcbin = bin.by_name(&format!("webrtc-{name}")).unwrap();

    let audio_decoder = bin.by_name("audio-decoder").unwrap();
    let video_decoder = bin.by_name("video-decoder").unwrap();

    let audio_fakesink = bin.by_name("audio-fakesink").unwrap();
    let video_fakesink = bin.by_name("video-fakesink").unwrap();

    webrtcbin.emit_by_name::<()>("set-remote-description", &[&None::<Promise>, &description]);

    let (send, recv) = oneshot::channel();
    let promise = Promise::with_change_func({
        let webrtcbin = webrtcbin.clone();

        move |result| {
            let answer = result.ok().flatten().and_then(|structure| {
                structure
                    .get::<gst_webrtc::WebRTCSessionDescription>("answer")
                    .ok()
            });

            if let Some(answer) = answer {
                // Set the answer as local description
                webrtcbin
                    .emit_by_name::<()>("set-local-description", &[&answer, &None::<gst::Promise>]);
                send.send(Some(answer)).unwrap();
            } else {
                send.send(None).unwrap();
            }
        }
    });

    webrtcbin.emit_by_name::<()>("create-answer", &[&None::<gst::Structure>, &promise]);

    let answer = if let Some(answer) = recv.await.ok().flatten() {
        answer.sdp().to_string()
    } else {
        todo!("error handling");
    };

    let audio_ghost_pad = gst::GhostPad::new(Some("audio_src"), gst::PadDirection::Src);
    let video_ghost_pad = gst::GhostPad::new(Some("video_src"), gst::PadDirection::Src);

    bin.add_pad(&audio_ghost_pad).unwrap();
    bin.add_pad(&video_ghost_pad).unwrap();

    audio_ghost_pad.connect(
        "linked",
        true,
        on_linked(
            audio_decoder.clone(),
            audio_fakesink.clone(),
            audio_ghost_pad.clone(),
        ),
    );

    audio_ghost_pad.connect(
        "unlinked",
        true,
        on_unlinked(
            audio_decoder.clone(),
            audio_fakesink.clone(),
            audio_ghost_pad.clone(),
        ),
    );

    video_ghost_pad.connect(
        "linked",
        true,
        on_linked(
            video_decoder.clone(),
            video_fakesink.clone(),
            video_ghost_pad.clone(),
        ),
    );

    video_ghost_pad.connect(
        "unlinked",
        true,
        on_unlinked(
            video_decoder.clone(),
            video_fakesink.clone(),
            video_ghost_pad.clone(),
        ),
    );

    let webrtcbin = WebRtcBin {
        bin,
        audio_src: audio_ghost_pad,
        video_src: video_ghost_pad,
        webrtcbin,
    };

    (webrtcbin, answer)
}
