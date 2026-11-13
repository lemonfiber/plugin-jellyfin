use super::{buffered, same_id, tuned};

const CHANNEL: &str = "adecd141257fda50dcb3439528c99432";

const SOURCE: &str = "f928d7e0acd678b1a1f517973a4e3238";

const LIVE_STREAM: &str = "e2329f4997b378e64ccf8fa396deb76e_af999c25a00715699361240d4c6c7a53_f928d7e0acd678b1a1f517973a4e3238";

const BUFFER: &str =
    "http://192.168.107.3:8096/LiveTv/LiveStreamFiles/15a6d5f479a34b4299268fb5e3f0bed6/stream.ts";

#[test]
fn the_buffer_is_the_path_at_jellyfin_of_its_own_copy_of_the_tuner() {
    assert_eq!(
        buffered(BUFFER).as_deref(),
        Some("/LiveTv/LiveStreamFiles/15a6d5f479a34b4299268fb5e3f0bed6/stream.ts")
    );
    assert_eq!(
        buffered("/LiveTv/LiveStreamFiles/15a6d5f479a34b4299268fb5e3f0bed6/stream.mkv").as_deref(),
        Some("/LiveTv/LiveStreamFiles/15a6d5f479a34b4299268fb5e3f0bed6/stream.mkv")
    );
}

#[test]
fn a_stream_jellyfin_did_not_buffer_itself_is_not_a_buffer() {
    for refused in [
        "http://files/channel.ts",
        "http://tuner:5004/auto/v7",
        "rtsp://camera/live",
        "http://h/LiveTv/LiveStreamFiles/15a6d5f479a34b4299268fb5e3f0bed6/stream.ts?x=1",
        "http://h/LiveTv/LiveStreamFiles/15a6d5f479a34b4299268fb5e3f0bed6/stream.ts#x",
        "http://h/LiveTv/LiveStreamFiles/15A6D5F479A34B4299268FB5E3F0BED6/stream.ts",
        "http://h/LiveTv/LiveStreamFiles/../stream.ts",
        "http://h/LiveTv/LiveStreamFiles/15a6d5f479a34b4299268fb5e3f0bed6/stream.",
        "http://h/LiveTv/LiveStreamFiles/15a6d5f479a34b4299268fb5e3f0bed6/stream.t/s",
        "http://h/x/LiveTv/LiveStreamFiles/15a6d5f479a34b4299268fb5e3f0bed6/stream.ts",
        "",
    ] {
        assert_eq!(
            buffered(refused),
            None,
            "{refused:?} was taken for a buffer"
        );
    }
}

#[test]
fn a_tuned_channel_answers_its_buffer_and_a_playlist_naming_its_live_stream() {
    let tuned = tuned(CHANNEL, SOURCE, LIVE_STREAM, Some(BUFFER));
    assert_eq!(
        tuned.as_ref().map(|tuned| tuned.closes_by.as_str()),
        Some(LIVE_STREAM)
    );
    assert_eq!(
        tuned.as_ref().map(|tuned| tuned.direct.as_str()),
        Some("/LiveTv/LiveStreamFiles/15a6d5f479a34b4299268fb5e3f0bed6/stream.ts")
    );
    assert_eq!(
        tuned.map(|tuned| tuned.transcoded),
        Some(format!(
            "/Videos/{CHANNEL}/master.m3u8?MediaSourceId={SOURCE}&LiveStreamId={LIVE_STREAM}&VideoCodec=h264&AudioCodec=aac&SegmentContainer=ts"
        ))
    );
}

#[test]
fn nothing_misshapen_is_written_into_a_path() {
    assert_eq!(tuned(CHANNEL, SOURCE, LIVE_STREAM, None), None);
    assert_eq!(tuned(CHANNEL, SOURCE, "planted&x=1", Some(BUFFER)), None);
    assert_eq!(tuned(CHANNEL, "../x", LIVE_STREAM, Some(BUFFER)), None);
    assert_eq!(tuned("../Users", SOURCE, LIVE_STREAM, Some(BUFFER)), None);
}

#[test]
fn a_channel_is_the_same_with_or_without_its_dashes() {
    assert!(same_id(CHANNEL, "adecd141-257f-da50-dcb3-439528c99432"));
    assert!(same_id(CHANNEL, "ADECD141257FDA50DCB3439528C99432"));
    assert!(!same_id(CHANNEL, SOURCE));
}
