use async_trait::async_trait;

use super::screening::{picture_path, ABSENT};
use super::{carrying, Jellyfin, AUTHORIZATION_HEADER};
use lemonfiber_ports::http::Method;
use lemonfiber_ports::service::{
    Addressee, Answers, BytePath, Declared, Declaring, Failure, Held, Locations, Naming, Picture,
    Ports, Presentation, Question, Statuses, Token, BASE, ITEM, TOKEN,
};

const PORT: u16 = 8096;

const GUARDED: u16 = 8920;

const PREFIXED: &str = "(?i)^(?:/emby|/mediabrowser)?";

const ITEM_SEGMENT: &str = "(?P<item>[^/]+)";

const ITEM_ID: &str = "(?i)^[0-9a-f]{8}-?[0-9a-f]{4}-?[0-9a-f]{4}-?[0-9a-f]{4}-?[0-9a-f]{12}$";

const HEX: &str = "^[0-9a-f]{32}$";

const TOKEN_PARAMETERS: [&str; 2] = ["ApiKey", "api_key"];

const ITEM_PARAMETER: &str = "itemId";

const LIVE_STREAM_PARAMETER: &str = "LiveStreamId";

const SOURCE_PARAMETER: &str = "MediaSourceId";

const MAY: Statuses = Statuses { from: 200, to: 299 };

const NO_SESSION: u16 = 401;

const NOT_FROM_THERE: u16 = 403;

const KEYED: &str = "(?:Videos|Audio|LiveTv/LiveStreamFiles|LiveTv/LiveRecordings)";

const UNKEYED: &str = "Videos/(?:ActiveEncodings|MergeVersions)$";

const BY_NAME: &str = "(?:Artists|Genres|MusicGenres|Persons|Studios)";

const STREAMED: &str = "VideoCodec=h264,hevc&AudioCodec=aac,mp3,ac3,eac3&SegmentContainer=mp4";

fn refusing() -> Vec<Naming> {
    [ITEM_PARAMETER, LIVE_STREAM_PARAMETER]
        .map(|parameter| Naming {
            parameter: parameter.to_owned(),
            held: Held::Refused,
        })
        .to_vec()
}

fn byte_path(route: &str, exempt: Vec<String>, naming: Vec<Naming>) -> BytePath {
    BytePath {
        pattern: format!("{PREFIXED}/{route}"),
        item: ITEM_ID.to_owned(),
        exempt,
        naming,
    }
}

fn declared() -> Declared {
    let mut trickplay = refusing();
    trickplay.push(Naming {
        parameter: SOURCE_PARAMETER.to_owned(),
        held: Held::Asked {
            shape: HEX.to_owned(),
        },
    });
    Declared {
        bytes: vec![
            byte_path(
                &format!("Videos/{ITEM_SEGMENT}/Trickplay/[^/]+/tiles\\.m3u8/?$"),
                Vec::new(),
                trickplay,
            ),
            byte_path(
                &format!("{KEYED}/{ITEM_SEGMENT}(?:/|$)"),
                vec![format!("{PREFIXED}/{UNKEYED}")],
                refusing(),
            ),
            byte_path(
                &format!("Items/{ITEM_SEGMENT}/(?:Images|Download|File)(?:/|$)"),
                Vec::new(),
                refusing(),
            ),
        ],
        refused: vec![
            format!("{PREFIXED}/{BY_NAME}/[^/]+/Images(?:/|$)"),
            format!("{PREFIXED}/Branding/Splashscreen/?$"),
        ],
        question: Some(Question {
            method: Method::Get,
            path: format!("/Items/{ITEM}"),
            to: Addressee::Upstream,
            answers: Answers {
                may: vec![MAY],
                may_not: vec![Statuses::only(ABSENT)],
                no_session: vec![Statuses::only(NO_SESSION)],
                not_from_there: vec![Statuses::only(NOT_FROM_THERE)],
            },
        }),
        token: Token {
            presented: Presentation::Header {
                name: AUTHORIZATION_HEADER.to_owned(),
                value: carrying(TOKEN),
            },
            shape: HEX.to_owned(),
            parameters: TOKEN_PARAMETERS.map(str::to_owned).to_vec(),
        },
        locations: Locations {
            stream: format!("{BASE}/Videos/{ITEM}/master.m3u8?MediaSourceId={ITEM}&{STREAMED}"),
            poster: format!("{BASE}{}", picture_path(ITEM, Picture::Poster)),
            backdrop: format!("{BASE}{}", picture_path(ITEM, Picture::Backdrop)),
        },
        ports: Ports {
            upstream: PORT,
            encrypted: Some(GUARDED),
        },
    }
}

#[async_trait]
impl Declaring for Jellyfin {
    async fn declared(&self) -> Result<Declared, Failure> {
        Ok(declared())
    }
}
