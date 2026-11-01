use term_core::{Cell, CoreParser, ScrollbackRing, Selection};

#[test]
fn test_print_and_cursor() {
    let mut parser = CoreParser::new(80, 24);
    parser.process_bytes(b"Hello, Terminal!");

    let line = parser.grid().get_display_line(0);
    let text: String = line.iter().map(|c| c.c).take(16).collect();
    assert_eq!(text, "Hello, Terminal!");
    assert_eq!(parser.grid().cursor_col, 16);
    assert_eq!(parser.grid().cursor_row, 0);
}

#[test]
fn test_ansi_colors_and_sgr() {
    let mut parser = CoreParser::new(80, 24);
    // Red text: \x1b[31m
    parser.process_bytes(b"\x1b[31mRed\x1b[0mDefault");

    let line = parser.grid().get_display_line(0);
    assert_eq!(line[0].c, 'R');
    assert_eq!(line[0].fg, (247, 118, 142)); // Tokyo Night Red
    assert_eq!(line[3].c, 'D');
    assert_eq!(line[3].fg, (226, 228, 234)); // Reset text: crisp off-white
}

#[test]
fn test_background_colors_truecolor_and_256() {
    let mut parser = CoreParser::new(80, 24);
    // 24-bit Truecolor background (RGB 100, 150, 200)
    parser.process_bytes(b"\x1b[48;2;100;150;200mBG\x1b[0m");

    let line = parser.grid().get_display_line(0);
    assert_eq!(line[0].c, 'B');
    assert_eq!(line[0].bg, (100, 150, 200));
    assert_eq!(line[1].c, 'G');
    assert_eq!(line[1].bg, (100, 150, 200));
    // After reset, default bg (black)
    assert_eq!(line[2].bg, (0, 0, 0));
}

#[test]
fn test_alternate_screen_buffer() {
    let mut parser = CoreParser::new(80, 24);
    parser.process_bytes(b"Primary Screen Text");
    assert!(!parser.use_alt);

    // Enter alt screen: \x1b[?1049h
    parser.process_bytes(b"\x1b[?1049h");
    assert!(parser.use_alt);
    parser.process_bytes(b"Alt Screen Text");

    let alt_line = parser.grid().get_display_line(0);
    let alt_text: String = alt_line.iter().map(|c| c.c).take(15).collect();
    assert_eq!(alt_text, "Alt Screen Text");

    // Exit alt screen: \x1b[?1049l
    parser.process_bytes(b"\x1b[?1049l");
    assert!(!parser.use_alt);

    let prim_line = parser.grid().get_display_line(0);
    let prim_text: String = prim_line.iter().map(|c| c.c).take(19).collect();
    assert_eq!(prim_text, "Primary Screen Text");
}

#[test]
fn test_selection_and_get_selected_text() {
    let mut parser = CoreParser::new(80, 24);
    parser.process_bytes(b"Select This Word Now");

    // Select "This" (col 7 to 10 on row 0)
    parser.grid_mut().selection = Some(Selection::new(7, 0, 10, 0));
    let selected = parser.grid().get_selected_text();
    assert_eq!(selected, Some("This".to_string()));
}

#[test]
fn test_damage_tracking() {
    let mut parser = CoreParser::new(80, 24);
    // Initial creation marks all dirty
    let damage = parser.grid_mut().extract_damage();
    assert_eq!(damage.len(), 24);

    // After extract, no damage
    let damage2 = parser.grid_mut().extract_damage();
    assert_eq!(damage2.len(), 0);

    // Printing marks only that line dirty
    parser.process_bytes(b"A");
    let damage3 = parser.grid_mut().extract_damage();
    assert_eq!(damage3.len(), 1);
    assert_eq!(damage3[0].row, 0);
}

#[test]
fn test_osc_133_block_tracking() {
    let mut parser = CoreParser::new(80, 24);
    parser.process_bytes(b"\x1b]133;A\x07echo 1\r\n");
    let line = parser.grid().get_display_line(0);
    assert_eq!(line[0].get_block_id(), Some(1));
}

#[test]
fn test_osc_title_setting() {
    let mut parser = CoreParser::new(80, 24);
    parser.process_bytes(b"\x1b]0;My Custom Terminal\x07");
    assert_eq!(parser.grid().title, Some("My Custom Terminal".to_string()));
}

#[test]
fn test_cursor_visibility_and_shapes() {
    let mut parser = CoreParser::new(80, 24);
    assert!(parser.grid().cursor_visible);
    assert_eq!(parser.grid().cursor_shape, term_core::CursorShape::Block);

    // Hide cursor: \x1b[?25l
    parser.process_bytes(b"\x1b[?25l");
    assert!(!parser.grid().cursor_visible);

    // Show cursor: \x1b[?25h
    parser.process_bytes(b"\x1b[?25h");
    assert!(parser.grid().cursor_visible);

    // DECSCUSR 3: Underline
    parser.process_bytes(b"\x1b[3 q");
    assert_eq!(parser.grid().cursor_shape, term_core::CursorShape::Underline);

    // DECSCUSR 5: Beam / Bar
    parser.process_bytes(b"\x1b[5 q");
    assert_eq!(parser.grid().cursor_shape, term_core::CursorShape::Beam);

    // DECSCUSR 2: Steady Block
    parser.process_bytes(b"\x1b[2 q");
    assert_eq!(parser.grid().cursor_shape, term_core::CursorShape::Block);
}

#[test]
fn test_modes_app_cursor_bracketed_paste_and_mouse() {
    let mut parser = CoreParser::new(80, 24);
    assert!(!parser.app_cursor_keys);
    assert!(!parser.bracketed_paste);
    assert!(!parser.mouse_tracking);
    assert!(!parser.mouse_sgr);

    // Enable Application Cursor Keys: \x1b[?1h
    parser.process_bytes(b"\x1b[?1h");
    assert!(parser.app_cursor_keys);

    // Disable Application Cursor Keys: \x1b[?1l
    parser.process_bytes(b"\x1b[?1l");
    assert!(!parser.app_cursor_keys);

    // Enable Bracketed Paste: \x1b[?2004h
    parser.process_bytes(b"\x1b[?2004h");
    assert!(parser.bracketed_paste);

    // Disable Bracketed Paste: \x1b[?2004l
    parser.process_bytes(b"\x1b[?2004l");
    assert!(!parser.bracketed_paste);

    // Enable Mouse Tracking (1000 & 1006 SGR): \x1b[?1000h\x1b[?1006h
    parser.process_bytes(b"\x1b[?1000h\x1b[?1006h");
    assert!(parser.mouse_tracking);
    assert!(parser.mouse_sgr);

    // Disable Mouse Tracking: \x1b[?1000l
    parser.process_bytes(b"\x1b[?1000l");
    assert!(!parser.mouse_tracking);
}

#[test]
fn test_char_editing_dch_ich_ech() {
    let mut parser = CoreParser::new(80, 24);
    // Write "ABCDEF"
    parser.process_bytes(b"ABCDEF");
    // Move cursor to column 2 (over 'C', 1-based col 3: \x1b[3G)
    parser.process_bytes(b"\x1b[3G");
    assert_eq!(parser.grid().cursor_col, 2);

    // DCH: Delete 2 chars ('C' and 'D') -> should become "ABEF"
    parser.process_bytes(b"\x1b[2P");
    let line = parser.grid().get_display_line(0);
    let text: String = line.iter().map(|c| c.c).take(4).collect();
    assert_eq!(text, "ABEF");

    // ICH: Insert 2 blank characters at cursor (over 'E') -> should become "AB  EF"
    parser.process_bytes(b"\x1b[2@");
    let line = parser.grid().get_display_line(0);
    let text: String = line.iter().map(|c| c.c).take(6).collect();
    assert_eq!(text, "AB  EF");

    // ECH: Erase 2 characters at cursor without shifting
    parser.process_bytes(b"\x1b[1G"); // col 0
    parser.process_bytes(b"\x1b[2X"); // erase 'A', 'B' -> should become "    EF"
    let line = parser.grid().get_display_line(0);
    let text: String = line.iter().map(|c| c.c).take(6).collect();
    assert_eq!(text, "    EF");
}

#[test]
fn test_cursor_save_restore_and_ri() {
    let mut parser = CoreParser::new(80, 24);
    // Position cursor at row 5, col 10 and set red color
    parser.process_bytes(b"\x1b[6;11H\x1b[31m");
    assert_eq!(parser.grid().cursor_row, 5);
    assert_eq!(parser.grid().cursor_col, 10);

    // Save cursor via DECSC: \x1b7
    parser.process_bytes(b"\x1b7");

    // Move to 0,0 and reset
    parser.process_bytes(b"\x1b[1;1H\x1b[0m");
    assert_eq!(parser.grid().cursor_row, 0);
    assert_eq!(parser.grid().cursor_col, 0);

    // Restore cursor via DECRC: \x1b8
    parser.process_bytes(b"\x1b8");
    assert_eq!(parser.grid().cursor_row, 5);
    assert_eq!(parser.grid().cursor_col, 10);
    assert_eq!(parser.grid().current_fg, (247, 118, 142)); // Restored Red

    // Reverse Index (RI): \x1bM -> cursor moves up to row 4
    parser.process_bytes(b"\x1bM");
    assert_eq!(parser.grid().cursor_row, 4);
}

#[test]
fn test_sgr_inverse_underline_italic_dim() {
    let mut parser = CoreParser::new(80, 24);
    // Print with Inverse (7), Underline (4), Italic (3), Dim (2)
    parser.process_bytes(b"\x1b[7;4;3;2mTest\x1b[0m");

    let line = parser.grid().get_display_line(0);
    let cell = line[0];
    assert_eq!(cell.c, 'T');
    assert!(cell.inverse);
    assert!(cell.underline);
    assert!(cell.italic);
    assert!(cell.dim);

    // After reset
    let reset_cell = line[4];
    assert!(!reset_cell.inverse);
    assert!(!reset_cell.underline);
    assert!(!reset_cell.italic);
    assert!(!reset_cell.dim);
}

#[test]
fn test_decawm_delayed_line_wrapping() {
    let mut parser = CoreParser::new(80, 24);
    // Print exactly 80 characters (cols = 80)
    let text = vec![b'A'; 80];
    parser.process_bytes(&text);

    // Cursor should be at last column of row 0, with wrap_next pending
    assert_eq!(parser.grid().cursor_row, 0);
    assert_eq!(parser.grid().cursor_col, 79);
    assert!(parser.grid().wrap_next);

    // If followed by \r\n (standard shell prompt or newline), it must NOT create a blank line
    parser.process_bytes(b"\r\n");
    assert_eq!(parser.grid().cursor_row, 1);
    assert_eq!(parser.grid().cursor_col, 0);
    assert!(!parser.grid().wrap_next);

    // If 81st character is printed directly without \r\n, it wraps to row 1, col 1
    let mut parser2 = CoreParser::new(80, 24);
    let text2 = vec![b'A'; 81];
    parser2.process_bytes(&text2);
    assert_eq!(parser2.grid().cursor_row, 1);
    assert_eq!(parser2.grid().cursor_col, 1);
}

#[test]
fn test_secondary_device_attributes_and_window_size() {
    let mut parser = CoreParser::new(80, 24);
    // Request Secondary Device Attributes (DA2): \x1b[>c
    parser.process_bytes(b"\x1b[>c");
    let resps = parser.drain_responses();
    assert_eq!(resps.len(), 1);
    assert_eq!(resps[0], b"\x1b[>0;115;0c");

    // Request terminal window size in characters: \x1b[18t
    parser.process_bytes(b"\x1b[18t");
    let resps2 = parser.drain_responses();
    assert_eq!(resps2.len(), 1);
    assert_eq!(resps2[0], b"\x1b[8;24;80t");
}

#[test]
fn test_osc_color_queries() {
    let mut parser = CoreParser::new(80, 24);
    // Query foreground color: \x1b]10;?\x07
    parser.process_bytes(b"\x1b]10;?\x07");
    // Query background color: \x1b]11;?\x07
    parser.process_bytes(b"\x1b]11;?\x07");

    let resps = parser.drain_responses();
    assert_eq!(resps.len(), 2);
    assert_eq!(resps[0], b"\x1b]10;rgb:e2e2/e4e4/eaea\x1b\\");
    assert_eq!(resps[1], b"\x1b]11;rgb:0000/0000/0000\x1b\\");
}

#[test]
fn test_custom_default_colors() {
    let mut parser = CoreParser::new(80, 24);
    parser.set_default_colors((200, 200, 200), (10, 10, 10));
    parser.process_bytes(b"\x1b]10;?\x07\x1b]11;?\x07");
    let resps = parser.drain_responses();
    assert_eq!(resps[0], b"\x1b]10;rgb:c8c8/c8c8/c8c8\x1b\\");
    assert_eq!(resps[1], b"\x1b]11;rgb:0a0a/0a0a/0a0a\x1b\\");
}

#[test]
fn test_clear_display_modes_2_and_3() {
    let mut parser = CoreParser::new(80, 24);
    // Move cursor to (10, 5) and write some text
    parser.process_bytes(b"\x1b[6;11HHello");
    assert_eq!(parser.grid().cursor_row, 5);
    assert_eq!(parser.grid().cursor_col, 15);

    // CSI 2 J clears screen, but MUST NOT move cursor
    parser.process_bytes(b"\x1b[2J");
    assert_eq!(parser.grid().cursor_row, 5);
    assert_eq!(parser.grid().cursor_col, 15);
    let cell = parser.grid().cells[5 * 80 + 10];
    assert_eq!(cell.c, ' ');

    // Push rows to scrollback
    for _ in 0..30 {
        parser.process_bytes(b"Line\r\n");
    }
    assert!(!parser.grid().scrollback.is_empty());

    // CSI 3 J clears scrollback, but does NOT reset active screen or cursor
    let cur_r = parser.grid().cursor_row;
    let cur_c = parser.grid().cursor_col;
    parser.process_bytes(b"\x1b[3J");
    assert_eq!(parser.grid().scrollback.len(), 0);
    assert_eq!(parser.grid().cursor_row, cur_r);
    assert_eq!(parser.grid().cursor_col, cur_c);
}

#[test]
fn test_bell_execution() {
    let mut parser = CoreParser::new(80, 24);
    assert!(!parser.bell);
    parser.process_bytes(b"\x07");
    assert!(parser.bell);
}

#[test]
fn test_get_url_at() {
    let mut parser = CoreParser::new(80, 24);
    parser.process_bytes(b"Visit https://github.com/project-terminal for docs.");

    // Clicking on "https" (col 6)
    let url1 = parser.grid().get_url_at(6, 0);
    assert_eq!(url1, Some("https://github.com/project-terminal".to_string()));

    // Clicking in middle of URL (col 25)
    let url2 = parser.grid().get_url_at(25, 0);
    assert_eq!(url2, Some("https://github.com/project-terminal".to_string()));

    // Clicking on "Visit" (col 2)
    let url3 = parser.grid().get_url_at(2, 0);
    assert_eq!(url3, None);

    // Clicking on trailing word "docs" (col 43)
    let url4 = parser.grid().get_url_at(43, 0);
    assert_eq!(url4, None);
}

#[test]
fn test_get_file_target_at() {
    let mut parser = CoreParser::new(80, 24);
    // T1: Proje kök dizinini açıkça cwd olarak ata (cwd-bağımsız test)
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let root_dir = std::path::Path::new(manifest_dir).parent().unwrap().parent().unwrap();
    parser.grid_mut().cwd = Some(root_dir.to_str().unwrap().to_string());

    parser.process_bytes(b"Error in Cargo.toml:12:4 occurred here");

    let target = parser.grid().get_file_target_at(12, 0);
    assert!(target.is_some());
    let t = target.unwrap();
    assert_eq!(t.path, "Cargo.toml");
    assert_eq!(t.line, Some(12));
    assert_eq!(t.col, Some(4));

    // Non-existent path returns None
    let nonexistent = parser.grid().get_file_target_at(30, 0); // "occurred"
    assert_eq!(nonexistent, None);
}

#[test]
fn test_command_block_jumping() {
    let mut parser = CoreParser::new(80, 5);
    // Command 1
    parser.process_bytes(b"\x1b]133;A\x07echo 1\r\n\x1b]133;C\x07Output 1\r\n\x1b]133;D;0\x07");
    // Command 2
    parser.process_bytes(b"\x1b]133;A\x07echo 2\r\n\x1b]133;C\x07Output 2\r\n\x1b]133;D;0\x07");
    // Command 3 with many output lines pushing into scrollback
    parser.process_bytes(b"\x1b]133;A\x07echo 3\r\n\x1b]133;C\x07");
    for i in 0..15 {
        parser.process_bytes(format!("Line {}\r\n", i).as_bytes());
    }
    parser.process_bytes(b"\x1b]133;D;0\x07");

    assert!(!parser.grid().scrollback.is_empty());

    // Jump to prev command
    let jumped = parser.grid_mut().scroll_to_prev_command();
    assert!(jumped);
    assert!(parser.grid().scroll_offset > 0);

    // Jump to next command
    let jumped_next = parser.grid_mut().scroll_to_next_command();
    assert!(jumped_next);
}

#[test]
fn test_search_query_and_matches() {
    let mut parser = CoreParser::new(80, 24);
    parser.process_bytes(b"First match ERROR found here.\r\nSecond line normal.\r\nThird match error here.");

    let grid = parser.grid_mut();
    grid.set_search_query(Some("error".to_string()));

    assert_eq!(grid.search_matches.len(), 2);
    assert_eq!(grid.active_match_idx, 0);

    // Active match is (start_col: 12, end_col: 17) on virtual line 0
    assert_eq!(grid.get_search_match_status(12, 0), Some(true));
    assert_eq!(grid.get_search_match_status(13, 0), Some(true));
    assert_eq!(grid.get_search_match_status(14, 0), Some(true));
    assert_eq!(grid.get_search_match_status(0, 0), None);

    // Navigate to next match
    grid.next_search_match();
    assert_eq!(grid.active_match_idx, 1);
    // Now match on row 2 is active
    assert_eq!(grid.get_search_match_status(12, 2), Some(true));
    // Match on row 0 is now inactive (Some(false))
    assert_eq!(grid.get_search_match_status(12, 0), Some(false));

    // Clear search
    grid.set_search_query(None);
    assert_eq!(grid.search_matches.len(), 0);
    assert_eq!(grid.search_query, None);
}

#[test]
fn test_get_last_command_output() {
    let mut parser = CoreParser::new(80, 24);
    // Command 1
    parser.process_bytes(b"\x1b]133;A\x07echo hello\r\n\x1b]133;C\x07Hello World!\r\n\x1b]133;D;0\x07");
    let output1 = parser.grid().get_last_command_output();
    assert_eq!(output1, Some("Hello World!".to_string()));

    // Command 2 with multiple lines
    parser.process_bytes(b"\x1b]133;A\x07cat file\r\n\x1b]133;C\x07line 1\r\nline 2\r\nline 3\r\n\x1b]133;D;0\x07");
    let output2 = parser.grid().get_last_command_output();
    assert_eq!(output2, Some("line 1\nline 2\nline 3".to_string()));
}

#[test]
fn test_cell_size_memory_footprint() {
    let size = std::mem::size_of::<Cell>();
    assert!(size <= 24, "Cell boyutu 24 baytı aşmamalı! Mevcut: {} bayt", size);
}

#[test]
fn test_scrollback_ring_buffer_fifo_and_recycling() {
    let mut ring = ScrollbackRing::with_capacity(3);
    assert_eq!(ring.len(), 0);
    assert!(ring.is_empty());

    let c1 = [Cell { c: '1', ..Cell::default() }];
    let c2 = [Cell { c: '2', ..Cell::default() }];
    let c3 = [Cell { c: '3', ..Cell::default() }];
    let c4 = [Cell { c: '4', ..Cell::default() }];
    let c5 = [Cell { c: '5', ..Cell::default() }];

    ring.push(&c1);
    ring.push(&c2);
    assert_eq!(ring.len(), 2);
    assert_eq!(ring[0][0].c, '1');
    assert_eq!(ring[1][0].c, '2');

    ring.push(&c3);
    assert_eq!(ring.len(), 3);
    assert_eq!(ring[0][0].c, '1');
    assert_eq!(ring[1][0].c, '2');
    assert_eq!(ring[2][0].c, '3');

    // Kapasite aşıldı: '1' üzerine '4' yazılır, baş kayar
    ring.push(&c4);
    assert_eq!(ring.len(), 3);
    assert_eq!(ring[0][0].c, '2');
    assert_eq!(ring[1][0].c, '3');
    assert_eq!(ring[2][0].c, '4');

    // Tekrar aşım: '2' üzerine '5' yazılır
    ring.push(&c5);
    assert_eq!(ring.len(), 3);
    assert_eq!(ring[0][0].c, '3');
    assert_eq!(ring[1][0].c, '4');
    assert_eq!(ring[2][0].c, '5');

    // Döngüsel iterasyon testi
    let chars: Vec<char> = ring.iter().map(|row| row[0].c).collect();
    assert_eq!(chars, vec!['3', '4', '5']);

    // Clear testi (kapasite korunur, eleman sayısı sıfırlanır)
    ring.clear();
    assert_eq!(ring.len(), 0);
    assert!(ring.is_empty());
}

#[test]
fn test_synchronized_output_mode_2026() {
    let mut parser = CoreParser::new(80, 24);
    assert!(!parser.synchronized_output);

    // BSU: Begin Synchronized Update (CSI ? 2026 h)
    parser.process_bytes(b"\x1b[?2026h");
    assert!(parser.synchronized_output);

    // DECRQM Query: Request Mode Status (CSI ? 2026 $ p)
    parser.process_bytes(b"\x1b[?2026$p");
    let resp = parser.drain_responses();
    assert_eq!(resp, vec![b"\x1b[?2026;1$y".to_vec()]); // 1 = set

    // ESU: End Synchronized Update (CSI ? 2026 l)
    parser.process_bytes(b"\x1b[?2026l");
    assert!(!parser.synchronized_output);

    // DECRQM Query again: should report reset (2)
    parser.process_bytes(b"\x1b[?2026$p");
    let resp2 = parser.drain_responses();
    assert_eq!(resp2, vec![b"\x1b[?2026;2$y".to_vec()]); // 2 = reset
}

#[test]
fn test_unicode_combining_characters() {
    let mut parser = CoreParser::new(80, 24);

    // 'e' + combining acute accent (\u{0301}) -> 'é'
    parser.process_bytes("e\u{0301}".as_bytes());
    assert_eq!(parser.grid().get_display_line(0)[0].c, 'é');
    assert_eq!(parser.grid().cursor_col, 1);

    // 'c' + combining cedilla (\u{0327}) -> 'ç'
    parser.process_bytes("c\u{0327}".as_bytes());
    assert_eq!(parser.grid().get_display_line(0)[1].c, 'ç');
    assert_eq!(parser.grid().cursor_col, 2);

    // 'g' + combining breve (\u{0306}) -> 'ğ'
    parser.process_bytes("g\u{0306}".as_bytes());
    assert_eq!(parser.grid().get_display_line(0)[2].c, 'ğ');
    assert_eq!(parser.grid().cursor_col, 3);

    // 's' + combining caron (\u{030C}) -> 'š'
    parser.process_bytes("s\u{030C}".as_bytes());
    assert_eq!(parser.grid().get_display_line(0)[3].c, 'š');

    // 'a' + combining macron (\u{0304}) -> 'ā'
    parser.process_bytes("a\u{0304}".as_bytes());
    assert_eq!(parser.grid().get_display_line(0)[4].c, 'ā');

    // '=' + combining slash (\u{0338}) -> '≠'
    parser.process_bytes("=\u{0338}".as_bytes());
    assert_eq!(parser.grid().get_display_line(0)[5].c, '≠');
}

#[test]
fn test_sixel_graphics_decoding() {
    let mut parser = CoreParser::new(80, 24);

    // DCS q (Sixel) tanımla: Renk #1 = Kırmızı, #1 seç, 6 piksel çiz (~), String Terminator (\x1b\)
    let sixel_stream = b"\x1bPq#1;2;100;0;0#1~-\x1b\\";
    parser.process_bytes(sixel_stream);

    let images = &parser.grid().images;
    assert_eq!(images.len(), 1, "Sixel resmi ayrıştırılmalı");
    let img = &images[0];
    assert!(img.width >= 1);
    assert_eq!(img.height, 6);
    // Kırmızı RGBA pikselleri kontrol et [255, 0, 0, 255]
    assert_eq!(&img.rgba[0..4], &[255, 0, 0, 255]);

    // DA1 sorgusu Sixel desteğini bildirmeli (kod 4)
    parser.process_bytes(b"\x1b[c");
    let responses = parser.drain_responses();
    assert_eq!(responses, vec![b"\x1b[?62;4;22c".to_vec()]);
}

#[test]
fn test_osc_52_clipboard_copy() {
    let mut parser = CoreParser::new(80, 24);

    // "Hello World" Base64: SGVsbG8gV29ybGQ=
    parser.process_bytes(b"\x1b]52;c;SGVsbG8gV29ybGQ=\x07");
    assert_eq!(parser.clipboard_store, Some("Hello World".to_string()));

    // Payload boşaltılınca None kalmalı
    let text = parser.clipboard_store.take();
    assert_eq!(text, Some("Hello World".to_string()));
    assert_eq!(parser.clipboard_store, None);
}

#[test]
fn test_sixel_memory_leak_prevention_and_scrolling() {
    let mut parser = CoreParser::new(80, 24);

    // 40 adet Sixel görseli gönder
    for _ in 0..40 {
        parser.process_bytes(b"\x1bPq#1;2;100;0;0#1~-\x1b\\");
    }

    // Kapasite sınırını (32) aşmamalı
    assert!(
        parser.grid().images.len() <= 32,
        "Görseller 32 adet ile sınırlandırılmalı, mevcut: {}",
        parser.grid().images.len()
    );

    // Ekran temizlendiğinde (CSI 2 J) tüm görseller bellekten silinmeli
    parser.process_bytes(b"\x1b[2J");
    assert_eq!(parser.grid().images.len(), 0, "CSI 2 J görselleri temizlemeli");
}

#[test]
fn test_osc_8_hyperlinks() {
    let mut parser = CoreParser::new(80, 24);

    // OSC 8 ile "Click Here" metnine https://rust-lang.org köprüsünü bağla
    let stream = b"\x1b]8;;https://rust-lang.org\x1b\\Click Here\x1b]8;;\x1b\\ Regular Text";
    parser.process_bytes(stream);

    // "Click Here" hücreleri (0..10 sütunları) köprü URL'sini döndürmeli
    for c in 0..10 {
        assert_eq!(
            parser.grid().get_url_at(c, 0),
            Some("https://rust-lang.org".to_string())
        );
    }

    // " Regular Text" hücreleri (10..23 sütunları) köprü döndürmemeli
    for c in 10..23 {
        assert_eq!(parser.grid().get_url_at(c, 0), None);
    }
}

#[test]
fn test_kitty_keyboard_protocol() {
    let mut parser = CoreParser::new(80, 24);
    assert_eq!(parser.keyboard_flags, 0);

    // 1. Sorgu: CSI ? u -> Yanıt: CSI ? 0 u
    parser.process_bytes(b"\x1b[?u");
    let resp = parser.drain_responses();
    assert_eq!(resp, vec![b"\x1b[?0u".to_vec()]);

    // 2. Bayrak İtme/Push: CSI > 3 u (flags = 3)
    parser.process_bytes(b"\x1b[>3u");
    assert_eq!(parser.keyboard_flags, 3);

    // 3. İkinci Bayrak İtme: CSI > 7 u (flags = 7)
    parser.process_bytes(b"\x1b[>7u");
    assert_eq!(parser.keyboard_flags, 7);

    // 4. Sorgu: CSI ? u -> Yanıt: CSI ? 7 u
    parser.process_bytes(b"\x1b[?u");
    let resp = parser.drain_responses();
    assert_eq!(resp, vec![b"\x1b[?7u".to_vec()]);

    // 5. Yığından Çıkarma/Pop: CSI < 1 u (önceki değer 3 olmalı)
    parser.process_bytes(b"\x1b[<1u");
    assert_eq!(parser.keyboard_flags, 3);

    // 6. Bir kez daha Pop: CSI < u (varsayılan 1 pop -> önceki değer 0 olmalı)
    parser.process_bytes(b"\x1b[<u");
    assert_eq!(parser.keyboard_flags, 0);

    // 7. Doğrudan Değer Atama: CSI = 5 ; 1 u
    parser.process_bytes(b"\x1b[=5;1u");
    assert_eq!(parser.keyboard_flags, 5);
}





#[test]
fn test_multiline_get_selected_text() {
    let mut parser = CoreParser::new(80, 24);
    parser.process_bytes(b"First line text\r\nSecond line text\r\nThird line text");

    // Satır 0 sütun 0'dan satır 2 sütun 4'e kadar seç
    parser.grid_mut().set_selection(Some(Selection::new(0, 0, 4, 2)));

    let selected = parser.grid().get_selected_text();
    assert!(selected.is_some());
    let text = selected.unwrap();
    assert_eq!(text, "First line text\nSecond line text\nThird");
}

#[test]
fn test_non_ascii_set_search_query() {
    let mut parser = CoreParser::new(80, 24);
    // Türkçe ve UTF-8 karakterler içeren metin: "Örnek terminal Türkçe desteği"
    parser.process_bytes("Örnek terminal Türkçe desteği".as_bytes());

    parser.grid_mut().set_search_query(Some("Türkçe".to_string()));
    assert_eq!(parser.grid().search_matches.len(), 1);

    let m = &parser.grid().search_matches[0];
    // "Örnek terminal " -> 15 karakter
    assert_eq!(m.start_col, 15);
    // "Türkçe" -> 6 karakter
    assert_eq!(m.end_col, 21);
}

#[test]
fn test_scrollback_ring_clear_then_push() {
    let mut ring = ScrollbackRing::with_capacity(3);
    let c1 = [Cell { c: 'A', ..Cell::default() }];
    let c2 = [Cell { c: 'B', ..Cell::default() }];
    let c3 = [Cell { c: 'C', ..Cell::default() }];
    let c4 = [Cell { c: 'D', ..Cell::default() }];

    ring.push(&c1);
    ring.push(&c2);
    ring.push(&c3);
    assert_eq!(ring.len(), 3);
    assert_eq!(ring[0][0].c, 'A');

    // clear sonrası tekrar ekleme (clear-then-push)
    ring.clear();
    assert_eq!(ring.len(), 0);
    assert!(ring.is_empty());

    ring.push(&c4);
    assert_eq!(ring.len(), 1);
    assert_eq!(ring[0][0].c, 'D');
}

#[test]
fn test_sixel_cell_height_and_spanned_rows() {
    let mut parser = CoreParser::new(80, 24);
    // Varsayılan cell_height = 16px iken: 30px görsel -> 30.div_ceil(16) = 2 satır
    let sixel_data = b"\x1bPq\"1;1;10;32#0;2;0;0;0#0!10~-!10~-!10~-!10~-!10~-\x1b\\";
    parser.process_bytes(sixel_data);

    assert_eq!(parser.grid().images.len(), 1);
    let img = &parser.grid().images[0];
    assert_eq!(img.height, 30);
    assert_eq!(img.rows_spanned, 2);

    // Y5: 20px hücre yüksekliği senaryosu
    let mut parser20 = CoreParser::new(80, 24);
    parser20.set_cell_height(20);
    parser20.process_bytes(sixel_data);

    assert_eq!(parser20.grid().images.len(), 1);
    let img20 = &parser20.grid().images[0];
    assert_eq!(img20.height, 30);
    // 30px görsel 20px hücre yüksekliğinde 30.div_ceil(20) = 2 satır kaplar
    assert_eq!(img20.rows_spanned, 2);

    // 45px görsel 20px hücrede 45.div_ceil(20) = 3 satır kaplar
    let mut parser_tall = CoreParser::new(80, 24);
    parser_tall.set_cell_height(20);
    // 8 band * 6 = 48px
    let sixel_tall = b"\x1bPq\"1;1;10;48#0;2;0;0;0#0!10~-!10~-!10~-!10~-!10~-!10~-!10~-!10~-\x1b\\";
    parser_tall.process_bytes(sixel_tall);
    assert_eq!(parser_tall.grid().images[0].rows_spanned, 3);
}

#[test]
fn test_turkish_istanbul_search_no_panic() {
    let mut parser = CoreParser::new(80, 24);
    parser.process_bytes("İstanbul hava durumu bugün çok güzel".as_bytes());

    // "İ" araması panic etmemeli ve 1 eşleşme bulmalı
    parser.grid_mut().set_search_query(Some("İ".to_string()));
    assert_eq!(parser.grid().search_matches.len(), 1);
    assert_eq!(parser.grid().search_matches[0].start_col, 0);
    assert_eq!(parser.grid().search_matches[0].end_col, 1);

    // "i" araması panic etmemeli
    parser.grid_mut().set_search_query(Some("i".to_string()));
    // "İstanbul" ve "bugün" içinde 'i' veya 'İ' eşleşmesi
    assert!(!parser.grid().search_matches.is_empty());
}
