//! Decides which clipboard changes must never be recorded.

/// Privacy markers other apps put on the clipboard alongside their data.
#[derive(Debug, Default, Clone, Copy)]
pub struct ClipFlags {
    /// `ExcludeClipboardContentFromMonitorProcessing` is present (any value).
    pub exclude_from_monitoring: bool,
    /// Value of `CanIncludeInClipboardHistory`, if present. Zero means "don't keep".
    pub can_include_in_history: Option<u32>,
    /// `Clipboard Viewer Ignore` is present (older convention, e.g. KeePass).
    pub viewer_ignore: bool,
}

/// True when the source app (usually a password manager) asked not to be recorded.
pub fn is_private(flags: &ClipFlags) -> bool {
    flags.exclude_from_monitoring || flags.can_include_in_history == Some(0) || flags.viewer_ignore
}

/// True when the copy came from an app on the user's exclusion list.
pub fn is_excluded_app(exe: Option<&str>, excluded: &[String]) -> bool {
    exe.is_some_and(|exe| excluded.iter().any(|x| x.eq_ignore_ascii_case(exe)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_clip_is_not_private() {
        assert!(!is_private(&ClipFlags::default()));
    }

    #[test]
    fn exclude_from_monitoring_is_private() {
        let f = ClipFlags {
            exclude_from_monitoring: true,
            ..Default::default()
        };
        assert!(is_private(&f));
    }

    #[test]
    fn can_include_in_history_respects_value() {
        let no = ClipFlags {
            can_include_in_history: Some(0),
            ..Default::default()
        };
        let yes = ClipFlags {
            can_include_in_history: Some(1),
            ..Default::default()
        };
        assert!(is_private(&no));
        assert!(!is_private(&yes));
    }

    #[test]
    fn viewer_ignore_is_private() {
        let f = ClipFlags {
            viewer_ignore: true,
            ..Default::default()
        };
        assert!(is_private(&f));
    }

    #[test]
    fn excluded_apps_match_case_insensitively() {
        let list = vec!["keepassxc.exe".to_string()];
        assert!(is_excluded_app(Some("KeePassXC.exe"), &list));
        assert!(!is_excluded_app(Some("notepad.exe"), &list));
        assert!(!is_excluded_app(None, &list));
    }
}
