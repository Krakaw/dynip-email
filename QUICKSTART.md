# Quick Start Guide

## 🚀 Get Started in 3 Steps

### 1. Start the Server

```bash
cargo run --release
```

You'll see:
```
🚀 Starting Temporary Mail Server
📝 Configuration:
  SMTP Port: 2525
  API Port: 3000
  Database: sqlite:emails.db
✅ Storage backend initialized
✅ SMTP server started on port 2525
📱 Web interface available at: http://localhost:3000
```

### 2. Open the Web Interface

Open your browser to: **http://localhost:3000**

### 3. Send a Test Email

In a new terminal:

```bash
python3 test_email.py
```

Or manually:

```bash
python3 -c "
import smtplib
from email.mime.text import MIMEText

msg = MIMEText('Hello from temporary mail!')
msg['Subject'] = 'Test Email'
msg['From'] = 'sender@example.com'
msg['To'] = 'test@example.com'

with smtplib.SMTP('localhost', 2525) as server:
    server.send_message(msg)
print('✅ Email sent!')
"
```

### 4. View the Email

1. In the web interface, enter: `test@example.com`
2. Click "Load Inbox"
3. Watch the email appear instantly! 🎉

## What Just Happened?

✅ **SMTP Server** accepted the email on port 2525  
✅ **SQLite Database** stored the email  
✅ **WebSocket** pushed the update to your browser in real-time  
✅ **Web UI** displayed the email instantly  

## Next Steps

- Try sending emails to different addresses
- Open multiple browser tabs with different email addresses
- Watch real-time updates as emails arrive
- Check out the full [README.md](README.md) for API details

## Troubleshooting

**Port already in use?**
```bash
export SMTP_PORT=2526
export API_PORT=3001
cargo run
```

**Can't send emails?**
- Make sure the server is running
- Check port 2525 is not blocked by firewall
- Verify Python 3 is installed for test script

## Architecture Overview

```
┌─────────────┐
│   Browser   │
│  (Port 3000)│
└──────┬──────┘
       │ WebSocket + REST API
       │
┌──────▼──────────────────┐
│   API Server (Axum)     │
│  - REST Endpoints       │
│  - WebSocket Handler    │
│  - Static File Server   │
└──────┬──────────────────┘
       │
       ├──────────────┐
       │              │
┌──────▼──────┐  ┌───▼────────────┐
│   Storage   │  │  SMTP Server   │
│  (SQLite)   │  │  (Port 2525)   │
└─────────────┘  └────────────────┘
```

## Features Showcase

🔄 **Real-time Updates**: WebSocket pushes emails instantly  
📧 **Any Address**: No registration, use any email address  
💾 **Persistent**: Emails stored in SQLite database  
🎨 **Modern UI**: Clean, responsive interface  
🔌 **Pluggable**: Swap storage backend via trait  

Enjoy your temporary mail server! 📬

