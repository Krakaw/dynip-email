#!/usr/bin/env python3
"""
Simple test script to send emails to the temporary mail server
"""

import smtplib
from email.mime.text import MIMEText
from email.mime.multipart import MIMEMultipart
import time

def send_test_email(to_address, subject, body):
    """Send a test email to the SMTP server"""
    msg = MIMEMultipart()
    msg['From'] = 'test-sender@example.com'
    msg['To'] = to_address
    msg['Subject'] = subject
    
    msg.attach(MIMEText(body, 'plain'))
    
    try:
        with smtplib.SMTP('localhost', 2525) as server:
            server.send_message(msg)
        print(f"✅ Email sent to {to_address}")
        return True
    except Exception as e:
        print(f"❌ Failed to send email: {e}")
        return False

if __name__ == '__main__':
    print("📧 Sending test emails to temporary mail server...\n")
    
    # Send a few test emails
    send_test_email(
        'test@example.com',
        'Welcome to Temporary Mail!',
        'This is your first test email. The server is working!'
    )
    
    time.sleep(1)
    
    send_test_email(
        'test@example.com',
        'Second Test Email',
        'This is another test email to demonstrate real-time updates.'
    )
    
    time.sleep(1)
    
    send_test_email(
        'another@example.com',
        'Email for different address',
        'This email is sent to a different address to test multiple inboxes.'
    )
    
    print("\n✨ Done! Check the web interface at http://localhost:3000")

