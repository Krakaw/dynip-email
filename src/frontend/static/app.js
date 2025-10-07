// Application state
let currentAddress = '';
let websocket = null;
let emails = [];
let selectedEmailId = null;

// DOM elements
const emailAddressInput = document.getElementById('emailAddress');
const loadEmailsBtn = document.getElementById('loadEmails');
const emailList = document.getElementById('emailList');
const emailDetail = document.getElementById('emailDetail');
const connectionStatus = document.getElementById('connectionStatus');
const emailCount = document.getElementById('emailCount');

// Event listeners
loadEmailsBtn.addEventListener('click', loadInbox);
emailAddressInput.addEventListener('keypress', (e) => {
    if (e.key === 'Enter') loadInbox();
});

// Load inbox for the specified email address
async function loadInbox() {
    const address = emailAddressInput.value.trim();
    
    if (!address) {
        alert('Please enter an email address');
        return;
    }
    
    currentAddress = address;
    
    // Close existing WebSocket connection
    if (websocket) {
        websocket.close();
    }
    
    // Fetch emails
    try {
        const response = await fetch(`/api/emails/${encodeURIComponent(address)}`);
        const data = await response.json();
        
        emails = data.emails || [];
        displayEmails(emails);
        updateEmailCount(emails.length);
        
        // Connect WebSocket
        connectWebSocket(address);
    } catch (error) {
        console.error('Failed to load emails:', error);
        emailList.innerHTML = '<div class="empty-state"><p>❌ Failed to load emails</p></div>';
    }
}

// Display emails in the list
function displayEmails(emailsToDisplay) {
    if (emailsToDisplay.length === 0) {
        emailList.innerHTML = '<div class="empty-state"><p>📭 No emails yet</p></div>';
        return;
    }
    
    emailList.innerHTML = emailsToDisplay
        .map(email => {
            const date = new Date(email.timestamp);
            const isNew = email.isNew || false;
            
            return `
                <div class="email-item ${email.id === selectedEmailId ? 'active' : ''}" 
                     data-email-id="${email.id}"
                     onclick="showEmailDetail('${email.id}')">
                    <div class="from">
                        ${escapeHtml(email.from)}
                        ${isNew ? '<span class="new-badge">NEW</span>' : ''}
                    </div>
                    <div class="subject">${escapeHtml(email.subject)}</div>
                    <div class="timestamp">${formatDate(date)}</div>
                </div>
            `;
        })
        .join('');
}

// Show email detail
function showEmailDetail(emailId) {
    const email = emails.find(e => e.id === emailId);
    if (!email) return;
    
    selectedEmailId = emailId;
    
    // Update active state in list
    document.querySelectorAll('.email-item').forEach(item => {
        item.classList.remove('active');
    });
    document.querySelector(`[data-email-id="${emailId}"]`)?.classList.add('active');
    
    // Display email detail
    const date = new Date(email.timestamp);
    
    // Determine if body is HTML or plain text
    const isHtml = email.body.includes('<') && email.body.includes('>');
    const bodyContent = isHtml
        ? `<iframe srcdoc="${escapeHtml(email.body)}"></iframe>`
        : `<pre style="white-space: pre-wrap; font-family: inherit;">${escapeHtml(email.body)}</pre>`;
    
    emailDetail.innerHTML = `
        <div class="email-header">
            <h2 class="email-subject">${escapeHtml(email.subject)}</h2>
            <div class="email-meta">
                <div class="email-meta-item">
                    <span class="email-meta-label">From:</span>
                    <span>${escapeHtml(email.from)}</span>
                </div>
                <div class="email-meta-item">
                    <span class="email-meta-label">To:</span>
                    <span>${escapeHtml(email.to)}</span>
                </div>
                <div class="email-meta-item">
                    <span class="email-meta-label">Date:</span>
                    <span>${formatDateFull(date)}</span>
                </div>
            </div>
        </div>
        <div class="email-body">
            ${bodyContent}
        </div>
    `;
    
    // Mark as read (remove new badge)
    if (email.isNew) {
        email.isNew = false;
        displayEmails(emails);
    }
}

// Connect to WebSocket for real-time updates
function connectWebSocket(address) {
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const wsUrl = `${protocol}//${window.location.host}/api/ws/${encodeURIComponent(address)}`;
    
    websocket = new WebSocket(wsUrl);
    
    websocket.onopen = () => {
        console.log('WebSocket connected');
        updateConnectionStatus(true);
    };
    
    websocket.onmessage = (event) => {
        try {
            const data = JSON.parse(event.data);
            
            if (data.type === 'connected') {
                console.log('Connected to WebSocket for:', data.address);
                return;
            }
            
            // New email received
            if (data.id) {
                console.log('New email received:', data);
                data.isNew = true;
                emails.unshift(data);
                displayEmails(emails);
                updateEmailCount(emails.length);
                
                // Show notification
                showNotification('📬 New email received!', `From: ${data.from}`);
            }
        } catch (error) {
            console.error('Failed to parse WebSocket message:', error);
        }
    };
    
    websocket.onerror = (error) => {
        console.error('WebSocket error:', error);
        updateConnectionStatus(false);
    };
    
    websocket.onclose = () => {
        console.log('WebSocket disconnected');
        updateConnectionStatus(false);
    };
}

// Update connection status indicator
function updateConnectionStatus(isConnected) {
    if (isConnected) {
        connectionStatus.classList.remove('offline');
        connectionStatus.classList.add('online');
        connectionStatus.querySelector('.status-text').textContent = 'Connected';
    } else {
        connectionStatus.classList.remove('online');
        connectionStatus.classList.add('offline');
        connectionStatus.querySelector('.status-text').textContent = 'Disconnected';
    }
}

// Update email count
function updateEmailCount(count) {
    emailCount.textContent = `${count} email${count !== 1 ? 's' : ''}`;
}

// Show browser notification
function showNotification(title, body) {
    if ('Notification' in window && Notification.permission === 'granted') {
        new Notification(title, { body });
    }
}

// Request notification permission on page load
if ('Notification' in window && Notification.permission === 'default') {
    Notification.requestPermission();
}

// Utility functions
function escapeHtml(text) {
    const map = {
        '&': '&amp;',
        '<': '&lt;',
        '>': '&gt;',
        '"': '&quot;',
        "'": '&#039;'
    };
    return text.replace(/[&<>"']/g, m => map[m]);
}

function formatDate(date) {
    const now = new Date();
    const diff = now - date;
    
    // Less than 1 minute
    if (diff < 60000) {
        return 'Just now';
    }
    
    // Less than 1 hour
    if (diff < 3600000) {
        const minutes = Math.floor(diff / 60000);
        return `${minutes} minute${minutes !== 1 ? 's' : ''} ago`;
    }
    
    // Less than 24 hours
    if (diff < 86400000) {
        const hours = Math.floor(diff / 3600000);
        return `${hours} hour${hours !== 1 ? 's' : ''} ago`;
    }
    
    // More than 24 hours
    return date.toLocaleDateString() + ' ' + date.toLocaleTimeString();
}

function formatDateFull(date) {
    return date.toLocaleDateString('en-US', {
        weekday: 'long',
        year: 'numeric',
        month: 'long',
        day: 'numeric',
        hour: '2-digit',
        minute: '2-digit'
    });
}

