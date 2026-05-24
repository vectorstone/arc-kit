use std::collections::HashMap;
use std::fs::File;
use std::io;
use std::io::IsTerminal;
use std::os::fd::{AsRawFd, RawFd};

use arc_core::agent::agent_spec;
use arc_core::provider::{ProviderInfo, supported_provider_agents};
use console::{Alignment, Key, measure_text_width, pad_str, style, truncate_str};

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProviderTab {
    agent: String,
    agent_display: String,
    provider_indexes: Vec<usize>,
    name_width: usize,
    has_active_provider: bool,
    default_row: usize,
}

struct CursorGuard;

impl Drop for CursorGuard {
    fn drop(&mut self) {
        let _ = show_cursor();
    }
}

struct RawTtyGuard {
    fd: RawFd,
    _tty: Option<File>,
    original: libc::termios,
}

impl RawTtyGuard {
    fn new() -> io::Result<Self> {
        let stdin = io::stdin();
        let (fd, tty) = if stdin.is_terminal() {
            (stdin.as_raw_fd(), None)
        } else {
            let tty = File::options().read(true).write(true).open("/dev/tty")?;
            let fd = tty.as_raw_fd();
            (fd, Some(tty))
        };
        let original = get_termios(fd)?;
        let mut raw = make_provider_raw(original);
        raw.c_cc[libc::VMIN] = 1;
        raw.c_cc[libc::VTIME] = 0;
        set_termios_now(fd, &raw)?;
        Ok(Self {
            fd,
            _tty: tty,
            original,
        })
    }

    fn fd(&self) -> RawFd {
        self.fd
    }
}

impl Drop for RawTtyGuard {
    fn drop(&mut self) {
        let _ = set_termios_now(self.fd, &self.original);
    }
}

fn make_provider_raw(original: libc::termios) -> libc::termios {
    let mut raw = original;
    raw.c_lflag &= !(libc::ECHO | libc::ICANON | libc::IEXTEN | libc::ISIG);
    raw.c_cc[libc::VMIN] = 1;
    raw.c_cc[libc::VTIME] = 0;
    raw
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProviderKeyAction {
    Cancel,
    Confirm,
    PrevRow,
    NextRow,
    PrevTab,
    NextTab,
    Noop,
}

pub fn select_provider(
    providers: &[ProviderInfo],
    active_providers: &HashMap<String, String>,
) -> io::Result<Option<ProviderInfo>> {
    let tabs = build_provider_tabs(providers, active_providers);
    if tabs.is_empty() {
        return Ok(None);
    }

    let mut tab = default_tab_index(&tabs);
    let mut rows: Vec<usize> = tabs.iter().map(|tab| tab.default_row).collect();
    let mut scrolls = vec![0usize; tabs.len()];
    let mut prev_drawn = 0usize;

    hide_cursor()?;
    let _cursor_guard = CursorGuard;

    loop {
        let (term_rows, cols) = terminal_size();
        let visible_rows = (term_rows as usize).saturating_sub(4).clamp(1, 12);
        let max_line_width = (cols as usize).saturating_sub(1).max(1);

        if prev_drawn > 0 {
            clear_drawn_block(prev_drawn)?;
        }

        let current_tab = &tabs[tab];
        let current_rows = current_tab.provider_indexes.len();
        if current_rows == 0 {
            show_cursor()?;
            return Ok(None);
        }

        rows[tab] = rows[tab].min(current_rows - 1);
        let mut scroll = scrolls[tab];
        if rows[tab] < scroll {
            scroll = rows[tab];
        } else if rows[tab] >= scroll + visible_rows {
            scroll = rows[tab] + 1 - visible_rows;
        }
        scrolls[tab] = scroll;

        write_clamped_line(format!("  {}", style("Provider").bold()), max_line_width)?;
        write_clamped_line(render_tab_line(&tabs, tab), max_line_width)?;

        let shown = current_rows.saturating_sub(scroll).min(visible_rows);
        for (pos, &provider_idx) in current_tab
            .provider_indexes
            .iter()
            .enumerate()
            .skip(scroll)
            .take(visible_rows)
        {
            let provider = &providers[provider_idx];
            let is_selected = pos == rows[tab];
            let is_active = active_providers
                .get(&current_tab.agent)
                .is_some_and(|name| name == &provider.name);
            write_clamped_line(
                render_provider_line(provider, current_tab.name_width, is_active, is_selected),
                max_line_width,
            )?;
        }

        write_clamped_line(
            render_hint_line(current_tab, current_rows, tabs.len() > 1),
            max_line_width,
        )?;

        prev_drawn = shown + 3;
        flush_stderr()?;
        let key = read_provider_key()?;
        match map_provider_key(key) {
            ProviderKeyAction::Cancel => {
                clear_drawn_block(prev_drawn)?;
                show_cursor()?;
                return Ok(None);
            }
            ProviderKeyAction::Confirm => {
                let provider_idx = current_tab.provider_indexes[rows[tab]];
                clear_drawn_block(prev_drawn)?;
                show_cursor()?;
                return Ok(Some(providers[provider_idx].clone()));
            }
            ProviderKeyAction::PrevRow => {
                rows[tab] = if rows[tab] == 0 {
                    current_rows - 1
                } else {
                    rows[tab] - 1
                };
            }
            ProviderKeyAction::NextRow => {
                rows[tab] = (rows[tab] + 1) % current_rows;
            }
            ProviderKeyAction::PrevTab => {
                tab = if tab == 0 { tabs.len() - 1 } else { tab - 1 };
            }
            ProviderKeyAction::NextTab => {
                tab = (tab + 1) % tabs.len();
            }
            ProviderKeyAction::Noop => {}
        }
    }
}

fn read_provider_key() -> io::Result<Key> {
    let tty = RawTtyGuard::new()?;
    read_tty_key(tty.fd())
}

fn get_termios(fd: libc::c_int) -> io::Result<libc::termios> {
    let mut termios = std::mem::MaybeUninit::uninit();
    let result = unsafe { libc::tcgetattr(fd, termios.as_mut_ptr()) };
    if result == 0 {
        Ok(unsafe { termios.assume_init() })
    } else {
        Err(io::Error::last_os_error())
    }
}

fn set_termios_now(fd: libc::c_int, termios: &libc::termios) -> io::Result<()> {
    set_termios(fd, libc::TCSANOW, termios)
}

fn set_termios(fd: libc::c_int, action: libc::c_int, termios: &libc::termios) -> io::Result<()> {
    let result = unsafe { libc::tcsetattr(fd, action, termios) };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

fn read_tty_key(fd: RawFd) -> io::Result<Key> {
    let byte = read_byte(fd)?;
    match byte {
        b'\x1b' => read_escape_key(fd),
        b'\n' | b'\r' => Ok(Key::Enter),
        b'\x7f' => Ok(Key::Backspace),
        b'\t' => Ok(Key::Tab),
        byte if byte.is_ascii() => Ok(Key::Char(byte as char)),
        first => read_utf8_key(fd, first),
    }
}

fn read_escape_key(fd: RawFd) -> io::Result<Key> {
    let mut seq = [0u8; 2];
    if read_fd(fd, &mut seq[..1])? == 0 {
        return Ok(Key::Escape);
    }
    if seq[0] != b'[' {
        return Ok(Key::Escape);
    }
    if read_fd(fd, &mut seq[1..2])? == 0 {
        return Ok(Key::Escape);
    }
    match seq[1] {
        b'A' => Ok(Key::ArrowUp),
        b'B' => Ok(Key::ArrowDown),
        b'C' => Ok(Key::ArrowRight),
        b'D' => Ok(Key::ArrowLeft),
        b'Z' => Ok(Key::BackTab),
        _ => Ok(Key::Escape),
    }
}

fn read_utf8_key(fd: RawFd, first: u8) -> io::Result<Key> {
    let len = if first & 0b1110_0000 == 0b1100_0000 {
        2
    } else if first & 0b1111_0000 == 0b1110_0000 {
        3
    } else if first & 0b1111_1000 == 0b1111_0000 {
        4
    } else {
        return Ok(Key::Unknown);
    };
    let mut buf = [0u8; 4];
    buf[0] = first;
    read_fd_exact(fd, &mut buf[1..len])?;
    match std::str::from_utf8(&buf[..len]) {
        Ok(value) => Ok(value.chars().next().map(Key::Char).unwrap_or(Key::Unknown)),
        Err(_) => Ok(Key::Unknown),
    }
}

fn read_byte(fd: RawFd) -> io::Result<u8> {
    let mut buf = [0u8; 1];
    read_fd_exact(fd, &mut buf)?;
    Ok(buf[0])
}

fn read_fd_exact(fd: RawFd, mut buf: &mut [u8]) -> io::Result<()> {
    while !buf.is_empty() {
        let read = read_fd(fd, buf)?;
        if read == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "terminal input closed",
            ));
        }
        buf = &mut buf[read..];
    }
    Ok(())
}

fn read_fd(fd: RawFd, buf: &mut [u8]) -> io::Result<usize> {
    loop {
        let read = unsafe { libc::read(fd, buf.as_mut_ptr().cast(), buf.len()) };
        if read >= 0 {
            return Ok(read as usize);
        }
        let err = io::Error::last_os_error();
        if err.kind() != io::ErrorKind::Interrupted {
            return Err(err);
        }
    }
}

fn map_provider_key(key: Key) -> ProviderKeyAction {
    match key {
        Key::Escape | Key::Char('q') | Key::Char('Q') => ProviderKeyAction::Cancel,
        Key::Enter | Key::Char('\n') | Key::Char('\r') => ProviderKeyAction::Confirm,
        Key::ArrowUp | Key::Char('k') | Key::Char('K') => ProviderKeyAction::PrevRow,
        Key::ArrowDown | Key::Char('j') | Key::Char('J') => ProviderKeyAction::NextRow,
        Key::ArrowLeft | Key::BackTab | Key::Char('h') | Key::Char('H') => {
            ProviderKeyAction::PrevTab
        }
        Key::ArrowRight | Key::Tab | Key::Char('l') | Key::Char('L') => ProviderKeyAction::NextTab,
        _ => ProviderKeyAction::Noop,
    }
}

fn build_provider_tabs(
    providers: &[ProviderInfo],
    active_providers: &HashMap<String, String>,
) -> Vec<ProviderTab> {
    supported_provider_agents()
        .into_iter()
        .filter_map(|agent| {
            let provider_indexes: Vec<usize> = providers
                .iter()
                .enumerate()
                .filter(|(_, provider)| provider.agent == agent)
                .map(|(index, _)| index)
                .collect();
            if provider_indexes.is_empty() {
                return None;
            }
            let name_width = provider_indexes
                .iter()
                .map(|&index| measure_text_width(&providers[index].display_name))
                .max()
                .unwrap_or(0);
            let active_row = active_providers.get(agent).and_then(|active_name| {
                provider_indexes
                    .iter()
                    .position(|&index| providers[index].name == *active_name)
            });
            let default_row = active_row.unwrap_or(0);

            Some(ProviderTab {
                agent: agent.to_string(),
                agent_display: agent_spec(agent)
                    .map(|spec| spec.display_name.to_string())
                    .unwrap_or_else(|| agent.to_string()),
                provider_indexes,
                name_width,
                has_active_provider: active_row.is_some(),
                default_row,
            })
        })
        .collect()
}

fn default_tab_index(tabs: &[ProviderTab]) -> usize {
    tabs.iter()
        .position(|tab| tab.has_active_provider)
        .unwrap_or(0)
}

fn render_tab_line(tabs: &[ProviderTab], active_tab: usize) -> String {
    let labels: Vec<String> = tabs
        .iter()
        .enumerate()
        .map(|(index, tab)| {
            let label = format!("[{}]", tab.agent_display);
            if index == active_tab {
                format!("{}", style(label).green().bold())
            } else {
                format!("{}", style(label).dim())
            }
        })
        .collect();
    format!("  {}", labels.join("  "))
}

fn render_provider_line(
    provider: &ProviderInfo,
    name_width: usize,
    is_active: bool,
    is_selected: bool,
) -> String {
    let marker = if is_active {
        format!("{}", style("✓").green())
    } else {
        " ".to_string()
    };

    let content = if provider.description.is_empty() {
        provider.display_name.clone()
    } else {
        let padded = pad_str(&provider.display_name, name_width, Alignment::Left, None);
        format!("{padded}  {}", provider.description)
    };

    if is_selected {
        format!(
            "  {} {} {}",
            style("❯").green(),
            marker,
            style(content).bold()
        )
    } else {
        format!("    {} {}", marker, style(content).dim())
    }
}

fn render_hint_line(tab: &ProviderTab, provider_count: usize, can_switch_tab: bool) -> String {
    let count_label = if provider_count == 1 {
        "1 provider".to_string()
    } else {
        format!("{provider_count} providers")
    };
    let nav = if can_switch_tab {
        "←→/tab or h/l switch agent  ↑↓ or j/k move  ↵ select  esc/q quit"
    } else {
        "↑↓ or j/k move  ↵ select  esc/q quit"
    };
    format!(
        "  {}",
        style(format!(
            "{}  ·  {}  ·  {}",
            tab.agent_display, count_label, nav
        ))
        .dim()
    )
}

fn truncation_tail(max_width: usize) -> &'static str {
    match max_width {
        0 => "",
        1 => ".",
        2 => "..",
        _ => "...",
    }
}

fn clamp_line_width(line: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }
    truncate_str(line, max_width, truncation_tail(max_width)).into_owned()
}

fn terminal_size() -> (u16, u16) {
    let mut size = std::mem::MaybeUninit::<libc::winsize>::uninit();
    let result = unsafe { libc::ioctl(2, libc::TIOCGWINSZ, size.as_mut_ptr()) };
    if result == 0 {
        let size = unsafe { size.assume_init() };
        if size.ws_row > 0 && size.ws_col > 0 {
            return (size.ws_row, size.ws_col);
        }
    }
    (24, 80)
}

fn write_clamped_line(line: String, max_width: usize) -> io::Result<()> {
    eprintln!("{}", clamp_line_width(&line, max_width));
    Ok(())
}

fn clear_drawn_block(lines: usize) -> io::Result<()> {
    eprint!("\x1b[{lines}A");
    for _ in 0..lines {
        eprint!("\r\x1b[2K\x1b[1B");
    }
    eprint!("\x1b[{lines}A");
    flush_stderr()
}

fn hide_cursor() -> io::Result<()> {
    eprint!("\x1b[?25l");
    flush_stderr()
}

fn show_cursor() -> io::Result<()> {
    eprint!("\x1b[?25h");
    flush_stderr()
}

fn flush_stderr() -> io::Result<()> {
    use std::io::Write;

    io::stderr().flush()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use arc_core::provider::{
        ClaudeProviderConfig, CodexProviderConfig, ProviderInfo, ProviderSettings,
    };
    use console::{Key, measure_text_width};

    use super::{
        ProviderKeyAction, build_provider_tabs, clamp_line_width, default_tab_index,
        map_provider_key, render_provider_line, render_tab_line,
    };

    fn provider(agent: &str, name: &str, display_name: &str, description: &str) -> ProviderInfo {
        ProviderInfo {
            name: name.to_string(),
            display_name: display_name.to_string(),
            description: description.to_string(),
            agent: agent.to_string(),
            settings: match agent {
                "claude" => ProviderSettings::Claude(ClaudeProviderConfig::default()),
                "codex" => ProviderSettings::Codex(CodexProviderConfig::default()),
                other => panic!("unexpected agent {other}"),
            },
        }
    }

    #[test]
    fn provider_tabs_follow_supported_agent_order() {
        let providers = vec![
            provider("codex", "official", "OpenAI", "official"),
            provider("claude", "proxy", "Mirror", "proxy"),
            provider("claude", "official", "Anthropic", "official"),
        ];

        let tabs = build_provider_tabs(&providers, &HashMap::new());

        assert_eq!(tabs.len(), 2);
        assert_eq!(tabs[0].agent, "claude");
        assert_eq!(tabs[0].provider_indexes, vec![1, 2]);
        assert_eq!(tabs[1].agent, "codex");
        assert_eq!(tabs[1].provider_indexes, vec![0]);
    }

    #[test]
    fn default_tab_prefers_agent_with_active_provider() {
        let providers = vec![
            provider("claude", "official", "Anthropic", ""),
            provider("codex", "proxy", "Mirror", ""),
            provider("codex", "official", "OpenAI", ""),
        ];
        let mut active = HashMap::new();
        active.insert("codex".to_string(), "proxy".to_string());

        let tabs = build_provider_tabs(&providers, &active);

        assert_eq!(tabs[1].default_row, 0);
        assert_eq!(default_tab_index(&tabs), 1);
    }

    #[test]
    fn render_provider_line_keeps_content_for_alignment() {
        let line = render_provider_line(
            &provider("claude", "official", "Anthropic", "official"),
            9,
            true,
            true,
        );

        assert!(line.contains("Anthropic"));
        assert!(line.contains("official"));
    }

    #[test]
    fn render_tab_line_marks_active_tab() {
        let providers = vec![
            provider("claude", "official", "Anthropic", ""),
            provider("codex", "official", "OpenAI", ""),
        ];

        let tabs = build_provider_tabs(&providers, &HashMap::new());
        let line = render_tab_line(&tabs, 1);

        assert!(line.contains("[Claude Code]"));
        assert!(line.contains("[Codex]"));
    }

    #[test]
    fn clamp_line_width_respects_terminal_width() {
        let clamped = clamp_line_width("0123456789", 5);

        assert_eq!(measure_text_width(&clamped), 5);
    }

    #[test]
    fn provider_key_map_supports_navigation_fallback_keys() {
        assert_eq!(map_provider_key(Key::Char('j')), ProviderKeyAction::NextRow);
        assert_eq!(map_provider_key(Key::Char('k')), ProviderKeyAction::PrevRow);
        assert_eq!(map_provider_key(Key::Char('h')), ProviderKeyAction::PrevTab);
        assert_eq!(map_provider_key(Key::Char('l')), ProviderKeyAction::NextTab);
        assert_eq!(map_provider_key(Key::Char('q')), ProviderKeyAction::Cancel);
        assert_eq!(
            map_provider_key(Key::Char('\r')),
            ProviderKeyAction::Confirm
        );
    }
}
