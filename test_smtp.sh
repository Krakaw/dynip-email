#!/bin/bash
# Test SMTP server with proper SMTP protocol

{
  sleep 0.5
  printf "HELO localhost\r\n"
  sleep 0.5
  printf "MAIL FROM:<test@example.com>\r\n"
  sleep 0.5
  printf "RCPT TO:<user@test.com>\r\n"
  sleep 0.5
  printf "DATA\r\n"
  sleep 0.5
  printf "Subject: Test Email\r\n"
  printf "From: test@example.com\r\n"
  printf "To: user@test.com\r\n"
  printf "\r\n"
  printf "This is a test message.\r\n"
  printf ".\r\n"
  sleep 0.5
  printf "QUIT\r\n"
  sleep 0.5
} | nc localhost 2525

