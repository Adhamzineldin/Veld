package lsp

import (
	"bufio"
	"encoding/json"
	"fmt"
	"io"
	"os"
	"strconv"
	"strings"
	"sync"
)

// Server is a JSON-RPC 2.0 LSP server that communicates over stdin/stdout.
type Server struct {
	reader  *bufio.Reader
	writer  io.Writer
	mu      sync.Mutex
	handler *Handler
}

// NewServer creates a new LSP server.
func NewServer() *Server {
	s := &Server{
		reader:  bufio.NewReader(os.Stdin),
		writer:  os.Stdout,
		handler: NewHandler(),
	}
	return s
}

// Run starts the server loop, reading requests and dispatching them.
func (s *Server) Run() error {
	for {
		msg, err := s.readMessage()
		if err != nil {
			if err == io.EOF {
				return nil
			}
			return err
		}

		var req Request
		if err := json.Unmarshal(msg, &req); err != nil {
			continue
		}

		s.dispatch(req)
	}
}

func (s *Server) readMessage() ([]byte, error) {
	contentLength := 0
	for {
		line, err := s.reader.ReadString('\n')
		if err != nil {
			return nil, err
		}
		line = strings.TrimSpace(line)
		if line == "" {
			break // end of headers
		}
		if strings.HasPrefix(line, "Content-Length:") {
			val := strings.TrimSpace(strings.TrimPrefix(line, "Content-Length:"))
			contentLength, _ = strconv.Atoi(val)
		}
	}

	if contentLength == 0 {
		return nil, fmt.Errorf("missing Content-Length header")
	}

	body := make([]byte, contentLength)
	_, err := io.ReadFull(s.reader, body)
	if err != nil {
		return nil, err
	}
	return body, nil
}

func (s *Server) sendResponse(resp Response) {
	s.mu.Lock()
	defer s.mu.Unlock()
	data, _ := json.Marshal(resp)
	header := fmt.Sprintf("Content-Length: %d\r\n\r\n", len(data))
	s.writer.Write([]byte(header))
	s.writer.Write(data)
}

func (s *Server) sendNotification(method string, params interface{}) {
	s.mu.Lock()
	defer s.mu.Unlock()
	notif := Notification{
		JSONRPC: "2.0",
		Method:  method,
		Params:  params,
	}
	data, _ := json.Marshal(notif)
	header := fmt.Sprintf("Content-Length: %d\r\n\r\n", len(data))
	s.writer.Write([]byte(header))
	s.writer.Write(data)
}

func (s *Server) dispatch(req Request) {
	switch req.Method {
	case "initialize":
		result := s.handler.Initialize()
		s.sendResponse(Response{JSONRPC: "2.0", ID: req.ID, Result: result})

	case "initialized":
		// No response needed

	case "shutdown":
		s.sendResponse(Response{JSONRPC: "2.0", ID: req.ID, Result: nil})

	case "exit":
		os.Exit(0)

	case "textDocument/didOpen":
		s.handleDidOpen(req)

	case "textDocument/didChange":
		s.handleDidChange(req)

	case "textDocument/didSave":
		s.handleDidSave(req)

	case "textDocument/completion":
		s.handleCompletion(req)

	case "textDocument/hover":
		s.handleHover(req)

	case "textDocument/definition":
		s.handleDefinition(req)
	}
}

func (s *Server) handleDidOpen(req Request) {
	paramsBytes, _ := json.Marshal(req.Params)
	var params struct {
		TextDocument TextDocumentItem `json:"textDocument"`
	}
	json.Unmarshal(paramsBytes, &params)
	s.handler.DidOpen(params.TextDocument.URI, params.TextDocument.Text)
	diags := s.handler.GetDiagnostics(params.TextDocument.URI)
	s.sendNotification("textDocument/publishDiagnostics", PublishDiagnosticsParams{
		URI:         params.TextDocument.URI,
		Diagnostics: diags,
	})
}

func (s *Server) handleDidChange(req Request) {
	paramsBytes, _ := json.Marshal(req.Params)
	var params struct {
		TextDocument   VersionedTextDocumentIdentifier  `json:"textDocument"`
		ContentChanges []TextDocumentContentChangeEvent `json:"contentChanges"`
	}
	json.Unmarshal(paramsBytes, &params)
	if len(params.ContentChanges) > 0 {
		s.handler.DidChange(params.TextDocument.URI, params.ContentChanges[0].Text)
		diags := s.handler.GetDiagnostics(params.TextDocument.URI)
		s.sendNotification("textDocument/publishDiagnostics", PublishDiagnosticsParams{
			URI:         params.TextDocument.URI,
			Diagnostics: diags,
		})
	}
}

func (s *Server) handleDidSave(req Request) {
	paramsBytes, _ := json.Marshal(req.Params)
	var params struct {
		TextDocument TextDocumentIdentifier `json:"textDocument"`
	}
	json.Unmarshal(paramsBytes, &params)
	diags := s.handler.GetDiagnostics(params.TextDocument.URI)
	s.sendNotification("textDocument/publishDiagnostics", PublishDiagnosticsParams{
		URI:         params.TextDocument.URI,
		Diagnostics: diags,
	})
}

func (s *Server) handleCompletion(req Request) {
	paramsBytes, _ := json.Marshal(req.Params)
	var params TextDocumentPositionParams
	json.Unmarshal(paramsBytes, &params)
	items := s.handler.Complete(params.TextDocument.URI, params.Position)
	s.sendResponse(Response{JSONRPC: "2.0", ID: req.ID, Result: items})
}

func (s *Server) handleHover(req Request) {
	paramsBytes, _ := json.Marshal(req.Params)
	var params TextDocumentPositionParams
	json.Unmarshal(paramsBytes, &params)
	hover := s.handler.Hover(params.TextDocument.URI, params.Position)
	s.sendResponse(Response{JSONRPC: "2.0", ID: req.ID, Result: hover})
}

func (s *Server) handleDefinition(req Request) {
	paramsBytes, _ := json.Marshal(req.Params)
	var params TextDocumentPositionParams
	json.Unmarshal(paramsBytes, &params)
	loc := s.handler.Definition(params.TextDocument.URI, params.Position)
	s.sendResponse(Response{JSONRPC: "2.0", ID: req.ID, Result: loc})
}
