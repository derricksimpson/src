package main

import (
	"fmt"
	"net/http"
)

type Server struct {
	Port int
	Name string
}

type Handler interface {
	ServeHTTP(w http.ResponseWriter, r *http.Request)
}

func NewServer(port int) *Server {
	return &Server{Port: port}
}

func (s *Server) Start() error {
	addr := fmt.Sprintf(":%d", s.Port)
	return http.ListenAndServe(addr, nil)
}

func (s *Server) Stop() {
	fmt.Println("stopping")
}

const MaxConnections = 100

var defaultPort = 8080

const (
	StatusOK    = 200
	StatusError = 500
)

type UserID string
