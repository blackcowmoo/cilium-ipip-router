# Project Structure

## Overview
- Project: cilium-ipip-router
- Location: /git/work
- Main source directory: src/
- Key modules:
  - src/controller/: Controller logic for Kubernetes node watching
  - src/main.rs: Application entry point
  - src/lib.rs: Library module declarations

## Code Organization
- src/controller/
  - builder.rs: ControllerBuilder for constructing controller instances
  - handle.rs: ControllerHandle for issuing commands to the controller
  - root.rs: Main Controller implementation with Kubernetes API integration
  - root_tests.rs: Unit tests for controller functionality
