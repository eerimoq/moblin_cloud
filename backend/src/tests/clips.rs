use super::*;

#[test]
fn twitch_clips_are_found_in_links() {
    let slug = "AwkwardHelplessSalamander-AbC_123xyz";
    for link in [
        format!("https://clips.twitch.tv/{slug}"),
        format!(" clips.twitch.tv/{slug}?tt_medium=x "),
        format!("https://www.twitch.tv/someone/clip/{slug}"),
        format!("https://m.twitch.tv/someone/clip/{slug}?filter=clips"),
        format!("https://clips.twitch.tv/embed?clip={slug}&parent=x"),
    ] {
        assert_eq!(twitch_slug(&link).as_deref(), Some(slug), "{link}");
    }
    for link in [
        "",
        "AwkwardHelplessSalamander",
        "https://www.twitch.tv/someone",
        "https://www.twitch.tv/someone/clip/",
        "https://www.twitch.tv/videos/123",
        "https://clips.twitch.tv/",
        "https://clips.twitch.tv/embed",
        "https://clips.twitch.tv/Bad%22Slug",
        "https://clips.twitch.tv.evil.example/Slug",
        "https://evil.example/someone/clip/Slug",
        "https://youtu.be/dQw4w9WgXcQ",
        "https://kick.com/someone/clips/clip_01M86Z1Z62360AM4D5KT55H4GS",
    ] {
        assert_eq!(twitch_slug(link), None, "{link}");
    }
}

#[test]
fn only_https_thumbnails_are_shown() {
    assert!(is_https("https://static-cdn.jtvnw.net/clip.jpg"));
    assert!(!is_https("http://static-cdn.jtvnw.net/clip.jpg"));
    assert!(!is_https("javascript:alert(1)"));
}
