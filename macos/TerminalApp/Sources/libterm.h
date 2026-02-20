// Auto-generated C header for libterm FFI.
// Swift imports this via bridging header.

#ifndef LIBTERM_H
#define LIBTERM_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct TermSession TermSession;

// Lifecycle
TermSession* term_session_new(uint32_t cols, uint32_t rows);
void term_session_free(TermSession* session);
int term_session_spawn_shell(TermSession* session, const char* shell);

// I/O
int term_session_read_pty(TermSession* session);
int term_session_write_pty(TermSession* session, const uint8_t* data, uint32_t len);
int term_session_pty_fd(const TermSession* session);

// Resize
void term_session_resize(TermSession* session, uint32_t cols, uint32_t rows,
                          uint32_t pixel_width, uint32_t pixel_height);

// Cell access
uint32_t term_session_cell_char(const TermSession* session, uint32_t row, uint32_t col);
uint32_t term_session_cell_fg(const TermSession* session, uint32_t row, uint32_t col);
uint32_t term_session_cell_bg(const TermSession* session, uint32_t row, uint32_t col);
uint8_t  term_session_cell_attr(const TermSession* session, uint32_t row, uint32_t col);

// Cursor & grid
void term_session_cursor_pos(const TermSession* session, uint32_t* out_row, uint32_t* out_col);
void term_session_grid_size(const TermSession* session, uint32_t* out_cols, uint32_t* out_rows);

// Title
char* term_session_title(const TermSession* session);
void term_string_free(char* s);

// Version
const char* libterm_version(void);

#ifdef __cplusplus
}
#endif

#endif // LIBTERM_H
