#!/usr/bin/env python3
"""
Simple test script to send emails to the temporary mail server
"""

import smtplib
from email.mime.text import MIMEText
from email.mime.multipart import MIMEMultipart
from email.mime.base import MIMEBase
from email import encoders
import time

def send_test_email(to_address, subject, body, attachment=None):
    """Send a test email to the SMTP server
    
    Args:
        to_address: Recipient email address
        subject: Email subject
        body: Email body text
        attachment: Optional tuple of (filename, content, mimetype)
    """
    msg = MIMEMultipart()
    msg['From'] = 'test-sender@example.com'
    msg['To'] = to_address
    msg['Subject'] = subject
    
    msg.attach(MIMEText(body, 'plain'))
    
    # Add attachment if provided
    if attachment:
        filename, content, mimetype = attachment
        part = MIMEBase(*mimetype.split('/'))
        part.set_payload(content)
        encoders.encode_base64(part)
        part.add_header('Content-Disposition', f'attachment; filename={filename}')
        msg.attach(part)
    
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
        'bob@dyn-ip.me',
        'Welcome to Temporary Mail!',
        'This is your first test email. The server is working!'
    )
    
    time.sleep(1)
    
    # Create a sample attachment
    attachment_content = b"""Hello! This is a sample text file attachment.

It contains some example data:
- Line 1
- Line 2
- Line 3

This demonstrates that the email server can handle attachments correctly.
"""
    
    send_test_email(
        'bob@dyn-ip.me',
        'Second Test Email with Attachment',
        'This is another test email with an attached file. Check it out!',
        attachment=('sample.txt', attachment_content, 'text/plain')
    )
    
    time.sleep(1)
    
    send_test_email(
        'another@example.com',
        'Email for different address',
        'This email is sent to a different address to test multiple inboxes.'
    )
    
    print("\n✨ Done! Check the web interface at http://localhost:3000")

