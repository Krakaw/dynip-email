// Application state
let currentAddress = '';
let websocket = null;
let emails = [];
let selectedEmailId = null;
let webhooks = [];
let currentTab = 'emails';

// DOM elements
const emailAddressInput = document.getElementById('emailAddress');
const loadEmailsBtn = document.getElementById('loadEmails');
const emailList = document.getElementById('emailList');
const webhookList = document.getElementById('webhookList');
const emailDetail = document.getElementById('emailDetail');
const connectionStatus = document.getElementById('connectionStatus');
const emailCount = document.getElementById('emailCount');
const themeToggle = document.getElementById('themeToggle');
const themeIcon = document.getElementById('themeIcon');
const inboxTab = document.getElementById('inboxTab');
const webhooksTab = document.getElementById('webhooksTab');

// Event listeners
loadEmailsBtn.addEventListener('click', loadInbox);
emailAddressInput.addEventListener('keypress', (e) => {
    if (e.key === 'Enter') loadInbox();
});
themeToggle.addEventListener('click', toggleTheme);
inboxTab.addEventListener('click', () => switchTab('emails'));
webhooksTab.addEventListener('click', () => switchTab('webhooks'));

// Theme management
function initTheme() {
    const savedTheme = localStorage.getItem('theme');
    const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
    
    if (savedTheme === 'dark' || (!savedTheme && prefersDark)) {
        enableDarkMode();
    } else {
        enableLightMode();
    }
}

function toggleTheme() {
    const isDark = document.documentElement.classList.contains('dark-mode');
    if (isDark) {
        enableLightMode();
    } else {
        enableDarkMode();
    }
}

function enableDarkMode() {
    document.documentElement.classList.add('dark-mode');
    themeIcon.textContent = '☀️';
    localStorage.setItem('theme', 'dark');
}

function enableLightMode() {
    document.documentElement.classList.remove('dark-mode');
    themeIcon.textContent = '🌙';
    localStorage.setItem('theme', 'light');
}

// Initialize from URL on page load
window.addEventListener('DOMContentLoaded', () => {
    // Initialize theme
    initTheme();
    
    // Load mailbox from URL if present
    const urlParams = new URLSearchParams(window.location.search);
    const mailbox = urlParams.get('mailbox');
    
    if (mailbox) {
        emailAddressInput.value = mailbox;
        loadInbox();
    }
});

// Load inbox for the specified email address
async function loadInbox() {
    const address = emailAddressInput.value.trim();
    
    if (!address) {
        alert('Please enter an address');
        return;
    }
    
    currentAddress = address;
    
    // Update URL with mailbox query parameter
    const url = new URL(window.location);
    url.searchParams.set('mailbox', address);
    window.history.pushState({}, '', url);
    
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
        
        // Load webhooks for this address
        loadWebhooks(address);
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
        : `<pre class="email-body-text">${escapeHtml(email.body)}</pre>`;
    
    // Build attachments HTML if any
    let attachmentsHtml = '';
    if (email.attachments && email.attachments.length > 0) {
        attachmentsHtml = `
            <div class="email-attachments">
                <h3>📎 Attachments (${email.attachments.length})</h3>
                <div class="attachments-list">
                    ${email.attachments.map(att => `
                        <div class="attachment-item">
                            <div class="attachment-info">
                                <span class="attachment-icon">${getFileIcon(att.content_type)}</span>
                                <div class="attachment-details">
                                    <div class="attachment-name">${escapeHtml(att.filename)}</div>
                                    <div class="attachment-meta">${formatFileSize(att.size)} • ${att.content_type}</div>
                                </div>
                            </div>
                            <button class="attachment-download" onclick="downloadAttachment('${email.id}', '${escapeHtml(att.filename)}', '${att.content_type}', '${att.content}')">
                                Download
                            </button>
                        </div>
                    `).join('')}
                </div>
            </div>
        `;
    }
    
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
        ${attachmentsHtml}
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
            
            if (data.type === 'Connected') {
                console.log('Connected to WebSocket for:', data.address);
                return;
            }
            
            // New email received
            if (data.type === 'Email') {
                const email = data;
                console.log('New email received:', email);
                email.isNew = true;
                emails.unshift(email);
                displayEmails(emails);
                updateEmailCount(emails.length);
                
                // Show notification
                showNotification('📬 New email received!', `From: ${email.from}`);
            }
            
            // Email deleted
            if (data.type === 'EmailDeleted') {
                const { id, address } = data;
                console.log('Email deleted:', id, 'for address:', address);
                
                // Remove the email from the local array
                const initialLength = emails.length;
                emails = emails.filter(email => email.id !== id);
                
                // Update the display if the email count changed
                if (emails.length !== initialLength) {
                    displayEmails(emails);
                    updateEmailCount(emails.length);
                    
                    // If the deleted email was currently selected, clear the detail view
                    if (selectedEmailId === id) {
                        selectedEmailId = null;
                        emailDetail.innerHTML = '<div class="empty-state"><p>📭 Select an email to view</p></div>';
                        
                        // Remove active state from all email items
                        document.querySelectorAll('.email-item').forEach(item => {
                            item.classList.remove('active');
                        });
                    }
                    
                    // Show notification
                    showNotification('🗑️ Email deleted', 'An email was removed due to retention policy');
                }
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

// Download attachment
function downloadAttachment(emailId, filename, contentType, base64Content) {
    try {
        // Decode base64 to binary
        const binaryString = atob(base64Content);
        const bytes = new Uint8Array(binaryString.length);
        for (let i = 0; i < binaryString.length; i++) {
            bytes[i] = binaryString.charCodeAt(i);
        }
        
        // Create blob and download
        const blob = new Blob([bytes], { type: contentType });
        const url = URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.href = url;
        a.download = filename;
        document.body.appendChild(a);
        a.click();
        document.body.removeChild(a);
        URL.revokeObjectURL(url);
    } catch (error) {
        console.error('Failed to download attachment:', error);
        alert('Failed to download attachment');
    }
}

// Get file icon based on content type
function getFileIcon(contentType) {
    if (contentType.startsWith('image/')) return '🖼️';
    if (contentType.startsWith('video/')) return '🎥';
    if (contentType.startsWith('audio/')) return '🎵';
    if (contentType.includes('pdf')) return '📄';
    if (contentType.includes('zip') || contentType.includes('compressed')) return '📦';
    if (contentType.includes('text/')) return '📝';
    if (contentType.includes('word') || contentType.includes('document')) return '📃';
    if (contentType.includes('sheet') || contentType.includes('excel')) return '📊';
    if (contentType.includes('presentation') || contentType.includes('powerpoint')) return '📽️';
    return '📎';
}

// Format file size
function formatFileSize(bytes) {
    if (bytes === 0) return '0 Bytes';
    const k = 1024;
    const sizes = ['Bytes', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return Math.round((bytes / Math.pow(k, i)) * 100) / 100 + ' ' + sizes[i];
}

// Tab switching
function switchTab(tab) {
    currentTab = tab;
    
    if (tab === 'emails') {
        inboxTab.classList.add('active');
        webhooksTab.classList.remove('active');
        emailList.style.display = 'block';
        webhookList.style.display = 'none';
    } else if (tab === 'webhooks') {
        webhooksTab.classList.add('active');
        inboxTab.classList.remove('active');
        emailList.style.display = 'none';
        webhookList.style.display = 'block';
    }
}

// Load webhooks for current address
async function loadWebhooks(address) {
    if (!address) return;
    
    try {
        const response = await fetch(`/api/webhooks/${encodeURIComponent(address)}`);
        const data = await response.json();
        
        webhooks = data.webhooks || [];
        displayWebhooks(webhooks);
    } catch (error) {
        console.error('Failed to load webhooks:', error);
        webhookList.innerHTML = '<div class="empty-state"><p>❌ Failed to load webhooks</p></div>';
    }
}

// Display webhooks
function displayWebhooks(webhooksToDisplay) {
    if (webhooksToDisplay.length === 0) {
        webhookList.innerHTML = `
            <div class="empty-state">
                <p>🔗 No webhooks configured for this mailbox</p>
                <button class="btn-primary" onclick="showAddWebhookForm()">Add Webhook</button>
            </div>
        `;
        return;
    }
    
    webhookList.innerHTML = `
        <div class="webhook-header">
            <h3>Webhooks (${webhooksToDisplay.length})</h3>
            <button class="btn-primary" onclick="showAddWebhookForm()">Add Webhook</button>
        </div>
        <div class="webhook-items">
            ${webhooksToDisplay.map(webhook => `
                <div class="webhook-item">
                    <div class="webhook-info">
                        <div class="webhook-url">${escapeHtml(webhook.webhook_url)}</div>
                        <div class="webhook-events">
                            Events: ${webhook.events.map(e => e.charAt(0).toUpperCase() + e.slice(1)).join(', ')}
                        </div>
                        <div class="webhook-status">
                            Status: <span class="status-${webhook.enabled ? 'enabled' : 'disabled'}">${webhook.enabled ? 'Enabled' : 'Disabled'}</span>
                        </div>
                    </div>
                    <div class="webhook-actions">
                        <button class="btn-secondary" onclick="testWebhook('${webhook.id}')">Test</button>
                        <button class="btn-secondary" onclick="editWebhook('${webhook.id}')">Edit</button>
                        <button class="btn-danger" onclick="deleteWebhook('${webhook.id}')">Delete</button>
                    </div>
                </div>
            `).join('')}
        </div>
    `;
}

// Show add webhook form
function showAddWebhookForm() {
    const formHtml = `
        <div class="webhook-form">
            <h3>Add New Webhook</h3>
            <form id="addWebhookForm">
                <div class="form-group">
                    <label for="webhookUrl">Webhook URL:</label>
                    <input type="url" id="webhookUrl" placeholder="http://localhost:3009 or https://example.com/webhook" required>
                    <small style="color: var(--text-secondary); font-size: 0.875rem; margin-top: 0.25rem; display: block;">
                        Enter the full URL including protocol (http:// or https://). For local testing, use http://localhost:PORT
                    </small>
                </div>
                <div class="form-group">
                    <label>Events to trigger:</label>
                    <div class="checkbox-group">
                        <label><input type="checkbox" name="events" value="arrival" checked> Email Arrival</label>
                        <label><input type="checkbox" name="events" value="deletion"> Email Deletion</label>
                        <label><input type="checkbox" name="events" value="read"> Email Read</label>
                    </div>
                </div>
                <div class="form-actions">
                    <button type="submit" class="btn-primary">Create Webhook</button>
                    <button type="button" class="btn-secondary" onclick="cancelAddWebhook()">Cancel</button>
                </div>
            </form>
        </div>
    `;
    
    webhookList.innerHTML = formHtml;
    
    document.getElementById('addWebhookForm').addEventListener('submit', async (e) => {
        e.preventDefault();
        await createWebhook();
    });
}

// Create webhook
async function createWebhook() {
    const url = document.getElementById('webhookUrl').value;
    const eventCheckboxes = document.querySelectorAll('input[name="events"]:checked');
    const events = Array.from(eventCheckboxes).map(cb => cb.value);
    
    if (events.length === 0) {
        alert('Please select at least one event');
        return;
    }
    
    try {
        const response = await fetch('/api/webhooks', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
            },
            body: JSON.stringify({
                mailbox_address: currentAddress,
                webhook_url: url,
                events: events
            })
        });
        
        if (response.ok) {
            await loadWebhooks(currentAddress);
        } else {
            const error = await response.text();
            alert('Failed to create webhook: ' + error);
        }
    } catch (error) {
        console.error('Failed to create webhook:', error);
        alert('Failed to create webhook');
    }
}

// Test webhook
async function testWebhook(webhookId) {
    try {
        const response = await fetch(`/api/webhook/${webhookId}/test`, {
            method: 'POST'
        });
        
        const result = await response.json();
        if (result.success) {
            alert('Webhook test successful!');
        } else {
            alert('Webhook test failed');
        }
    } catch (error) {
        console.error('Failed to test webhook:', error);
        alert('Failed to test webhook');
    }
}

// Edit webhook
function editWebhook(webhookId) {
    const webhook = webhooks.find(w => w.id === webhookId);
    if (!webhook) return;
    
    const formHtml = `
        <div class="webhook-form">
            <h3>Edit Webhook</h3>
            <form id="editWebhookForm">
                <div class="form-group">
                    <label for="editWebhookUrl">Webhook URL:</label>
                    <input type="url" id="editWebhookUrl" value="${escapeHtml(webhook.webhook_url)}" required>
                    <small style="color: var(--text-secondary); font-size: 0.875rem; margin-top: 0.25rem; display: block;">
                        Enter the full URL including protocol (http:// or https://). For local testing, use http://localhost:PORT
                    </small>
                </div>
                <div class="form-group">
                    <label>Events to trigger:</label>
                    <div class="checkbox-group">
                        <label><input type="checkbox" name="editEvents" value="arrival" ${webhook.events.includes('arrival') ? 'checked' : ''}> Email Arrival</label>
                        <label><input type="checkbox" name="editEvents" value="deletion" ${webhook.events.includes('deletion') ? 'checked' : ''}> Email Deletion</label>
                        <label><input type="checkbox" name="editEvents" value="read" ${webhook.events.includes('read') ? 'checked' : ''}> Email Read</label>
                    </div>
                </div>
                <div class="form-group">
                    <label>
                        <input type="checkbox" name="enabled" ${webhook.enabled ? 'checked' : ''}> Enabled
                    </label>
                </div>
                <div class="form-actions">
                    <button type="submit" class="btn-primary">Update Webhook</button>
                    <button type="button" class="btn-secondary" onclick="cancelEditWebhook()">Cancel</button>
                </div>
            </form>
        </div>
    `;
    
    webhookList.innerHTML = formHtml;
    
    document.getElementById('editWebhookForm').addEventListener('submit', async (e) => {
        e.preventDefault();
        await updateWebhook(webhookId);
    });
}

// Update webhook
async function updateWebhook(webhookId) {
    const url = document.getElementById('editWebhookUrl').value;
    const eventCheckboxes = document.querySelectorAll('input[name="editEvents"]:checked');
    const events = Array.from(eventCheckboxes).map(cb => cb.value);
    const enabled = document.querySelector('input[name="enabled"]').checked;
    
    if (events.length === 0) {
        alert('Please select at least one event');
        return;
    }
    
    try {
        const response = await fetch(`/api/webhook/${webhookId}`, {
            method: 'PUT',
            headers: {
                'Content-Type': 'application/json',
            },
            body: JSON.stringify({
                webhook_url: url,
                events: events,
                enabled: enabled
            })
        });
        
        if (response.ok) {
            await loadWebhooks(currentAddress);
        } else {
            const error = await response.text();
            alert('Failed to update webhook: ' + error);
        }
    } catch (error) {
        console.error('Failed to update webhook:', error);
        alert('Failed to update webhook');
    }
}

// Delete webhook
async function deleteWebhook(webhookId) {
    if (!confirm('Are you sure you want to delete this webhook?')) {
        return;
    }
    
    try {
        const response = await fetch(`/api/webhook/${webhookId}`, {
            method: 'DELETE'
        });
        
        if (response.ok) {
            await loadWebhooks(currentAddress);
        } else {
            alert('Failed to delete webhook');
        }
    } catch (error) {
        console.error('Failed to delete webhook:', error);
        alert('Failed to delete webhook');
    }
}

// Cancel add webhook
function cancelAddWebhook() {
    displayWebhooks(webhooks);
}

// Cancel edit webhook
function cancelEditWebhook() {
    displayWebhooks(webhooks);
}

