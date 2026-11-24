use super::*;

#[test]
fn video_ids_are_found_in_links() {
    for link in [
        "dQw4w9WgXcQ",
        " https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=43s ",
        "https://youtube.com/watch?feature=shared&v=dQw4w9WgXcQ",
        "http://m.youtube.com/watch?v=dQw4w9WgXcQ",
        "https://music.youtube.com/watch?v=dQw4w9WgXcQ",
        "https://youtu.be/dQw4w9WgXcQ?t=43",
        "youtu.be/dQw4w9WgXcQ",
        "www.youtube.com/watch?v=dQw4w9WgXcQ",
        "https://www.youtube.com/shorts/dQw4w9WgXcQ",
        "https://www.youtube.com/live/dQw4w9WgXcQ?si=x",
        "https://www.youtube.com/embed/dQw4w9WgXcQ?start=43",
        "https://WWW.YouTube.com/watch?v=dQw4w9WgXcQ",
    ] {
        assert_eq!(video_id(link).as_deref(), Some("dQw4w9WgXcQ"), "{link}");
    }
    for link in [
        "",
        "dQw4w9WgXc",
        "dQw4w9WgXcQ!",
        "https://www.youtube.com/watch?v=dQw4w9WgXc",
        "https://www.youtube.com/watch?v=dQw4w9WgXcQ%22",
        "https://www.youtube.com/watch",
        "https://www.youtube.com/clip/UgkxdQw4w9WgXcQ",
        "https://www.youtube.com/@RickAstleyYT",
        "https://www.youtube.com/",
        "https://evil.example/watch?v=dQw4w9WgXcQ",
        "https://youtube.com.evil.example/watch?v=dQw4w9WgXcQ",
        "https://evil.example/?https://youtu.be/dQw4w9WgXcQ",
        "https://vimeo.com/dQw4w9WgXcQ",
    ] {
        assert_eq!(video_id(link), None, "{link}");
    }
}

#[test]
fn start_times_are_found_in_links() {
    for (link, seconds) in [
        ("https://youtu.be/dQw4w9WgXcQ?t=43", Some(43)),
        (
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=43s",
            Some(43),
        ),
        (
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=1m13s",
            Some(73),
        ),
        (
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=1h2m3s",
            Some(3723),
        ),
        (
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=2m",
            Some(120),
        ),
        ("https://www.youtube.com/embed/dQw4w9WgXcQ?start=7", Some(7)),
        ("youtu.be/dQw4w9WgXcQ?t=5", Some(5)),
        ("https://www.youtube.com/watch?v=dQw4w9WgXcQ", None),
        ("https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=soon", None),
        ("https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=1m5", None),
        (
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=99999999999h",
            None,
        ),
        ("dQw4w9WgXcQ", None),
    ] {
        assert_eq!(start(link), seconds, "{link}");
    }
}
