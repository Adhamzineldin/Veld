// Package errors provides structured, typed error types for the Veld pipeline.
//
// Every stage of the pipeline (lexer, parser, validator, emitter, config) returns
// one of these types so callers can programmatically inspect error details — file,
// line, kind — without regex-parsing error strings.
package errors

import "fmt"

// Kind classifies the pipeline stage that produced an error.
type Kind int

const (
	KindConfig     Kind = iota // config file loading / merging
	KindParse                  // lexer or parser errors
	KindValidation             // semantic validation errors
	KindEmit                   // code generation errors
	KindLint                   // linter findings promoted to errors
	KindIO                     // file system / network errors
)

func (k Kind) String() string {
	switch k {
	case KindConfig:
		return "config"
	case KindParse:
		return "parse"
	case KindValidation:
		return "validation"
	case KindEmit:
		return "emit"
	case KindLint:
		return "lint"
	case KindIO:
		return "io"
	default:
		return "unknown"
	}
}

// VeldError is the base structured error type used throughout Veld.
type VeldError struct {
	Kind    Kind   // pipeline stage
	File    string // source file (absolute path or basename)
	Line    int    // 1-based line number (0 = unknown)
	Message string // human-readable description
	Cause   error  // wrapped underlying error (may be nil)
}

func (e *VeldError) Error() string {
	prefix := ""
	if e.File != "" && e.Line > 0 {
		prefix = fmt.Sprintf("%s:%d: ", e.File, e.Line)
	} else if e.File != "" {
		prefix = e.File + ": "
	} else if e.Line > 0 {
		prefix = fmt.Sprintf("line %d: ", e.Line)
	}
	return prefix + e.Message
}

func (e *VeldError) Unwrap() error { return e.Cause }

// ── Convenience constructors ──────────────────────────────────────────────────

// NewParseError creates a parse-stage error.
func NewParseError(file string, line int, msg string) *VeldError {
	return &VeldError{Kind: KindParse, File: file, Line: line, Message: msg}
}

// NewValidationError creates a validation-stage error.
func NewValidationError(file string, line int, msg string) *VeldError {
	return &VeldError{Kind: KindValidation, File: file, Line: line, Message: msg}
}

// NewEmitError creates an emit-stage error.
func NewEmitError(file string, msg string, cause error) *VeldError {
	return &VeldError{Kind: KindEmit, File: file, Message: msg, Cause: cause}
}

// NewConfigError creates a config-stage error.
func NewConfigError(msg string, cause error) *VeldError {
	return &VeldError{Kind: KindConfig, Message: msg, Cause: cause}
}

// NewIOError creates an I/O error.
func NewIOError(file string, msg string, cause error) *VeldError {
	return &VeldError{Kind: KindIO, File: file, Message: msg, Cause: cause}
}

// ── Multi-error collector ─────────────────────────────────────────────────────

// List collects multiple errors and implements the error interface itself.
type List struct {
	Errors []*VeldError
}

func (l *List) Add(e *VeldError) { l.Errors = append(l.Errors, e) }
func (l *List) Len() int         { return len(l.Errors) }
func (l *List) HasErrors() bool  { return len(l.Errors) > 0 }

func (l *List) Error() string {
	if len(l.Errors) == 0 {
		return "no errors"
	}
	if len(l.Errors) == 1 {
		return l.Errors[0].Error()
	}
	return fmt.Sprintf("%d errors (first: %s)", len(l.Errors), l.Errors[0].Error())
}

// AsErrors returns the list as []error for backward compatibility.
func (l *List) AsErrors() []error {
	out := make([]error, len(l.Errors))
	for i, e := range l.Errors {
		out[i] = e
	}
	return out
}

// AsVeldError attempts to extract a *VeldError from any error.
// Returns nil if the error is not a *VeldError.
func AsVeldError(err error) *VeldError {
	if ve, ok := err.(*VeldError); ok {
		return ve
	}
	return nil
}
