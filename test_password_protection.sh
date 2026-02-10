#!/bin/bash

# Test script for mailbox password protection feature

set -e

API_URL="http://localhost:3000"
MAILBOX="test-secure"

echo "🧪 Testing Mailbox Password Protection"
echo "======================================="
echo ""

# Start the server in the background
echo "📡 Starting server..."
cargo run --quiet &
SERVER_PID=$!

# Wait for server to start
sleep 5

# Cleanup function
cleanup() {
    echo ""
    echo "🧹 Cleaning up..."
    kill $SERVER_PID 2>/dev/null || true
    rm -f emails.db
}

trap cleanup EXIT

echo ""
echo "1️⃣  Checking initial mailbox status (should be unlocked)..."
RESPONSE=$(curl -s "$API_URL/api/mailbox/$MAILBOX/status")
echo "   Response: $RESPONSE"
IS_LOCKED=$(echo $RESPONSE | grep -o '"is_locked":false' || echo "")
if [ -n "$IS_LOCKED" ]; then
    echo "   ✅ Mailbox is unlocked (as expected)"
else
    echo "   ❌ FAILED: Expected unlocked mailbox"
    exit 1
fi

echo ""
echo "2️⃣  Attempting to access emails without password (should succeed)..."
RESPONSE=$(curl -s "$API_URL/api/emails/$MAILBOX")
echo "   Response: $RESPONSE"
if echo "$RESPONSE" | grep -q '"emails"'; then
    echo "   ✅ Access granted (as expected)"
else
    echo "   ❌ FAILED: Should allow access to unlocked mailbox"
    exit 1
fi

echo ""
echo "3️⃣  Claiming mailbox with password..."
RESPONSE=$(curl -s -X POST "$API_URL/api/mailbox/$MAILBOX/claim" \
    -H "Content-Type: application/json" \
    -d '{"password":"mysecretpassword"}')
echo "   Response: $RESPONSE"
if echo "$RESPONSE" | grep -q "claimed successfully"; then
    echo "   ✅ Mailbox claimed successfully"
else
    echo "   ❌ FAILED: Could not claim mailbox"
    exit 1
fi

echo ""
echo "4️⃣  Checking mailbox status after claim (should be locked)..."
RESPONSE=$(curl -s "$API_URL/api/mailbox/$MAILBOX/status")
echo "   Response: $RESPONSE"
IS_LOCKED=$(echo $RESPONSE | grep -o '"is_locked":true' || echo "")
if [ -n "$IS_LOCKED" ]; then
    echo "   ✅ Mailbox is now locked (as expected)"
else
    echo "   ❌ FAILED: Expected locked mailbox"
    exit 1
fi

echo ""
echo "5️⃣  Attempting to access without password (should fail)..."
RESPONSE=$(curl -s "$API_URL/api/emails/$MAILBOX")
echo "   Response: $RESPONSE"
if echo "$RESPONSE" | grep -q "password protected"; then
    echo "   ✅ Access denied (as expected)"
else
    echo "   ❌ FAILED: Should require password"
    exit 1
fi

echo ""
echo "6️⃣  Attempting to access with wrong password (should fail)..."
RESPONSE=$(curl -s "$API_URL/api/emails/$MAILBOX?password=wrongpassword")
echo "   Response: $RESPONSE"
if echo "$RESPONSE" | grep -q "verification error"; then
    echo "   ✅ Wrong password rejected (as expected)"
else
    echo "   ❌ FAILED: Should reject wrong password"
    exit 1
fi

echo ""
echo "7️⃣  Accessing with correct password (should succeed)..."
RESPONSE=$(curl -s "$API_URL/api/emails/$MAILBOX?password=mysecretpassword")
echo "   Response: $RESPONSE"
if echo "$RESPONSE" | grep -q '"emails"'; then
    echo "   ✅ Access granted with correct password (as expected)"
else
    echo "   ❌ FAILED: Should allow access with correct password"
    exit 1
fi

echo ""
echo "8️⃣  Attempting to claim already locked mailbox (should fail)..."
RESPONSE=$(curl -s -X POST "$API_URL/api/mailbox/$MAILBOX/claim" \
    -H "Content-Type: application/json" \
    -d '{"password":"anotherpassword"}')
echo "   Response: $RESPONSE"
if echo "$RESPONSE" | grep -q "already claimed"; then
    echo "   ✅ Re-claim prevented (as expected)"
else
    echo "   ❌ FAILED: Should not allow re-claiming locked mailbox"
    exit 1
fi

echo ""
echo "✅ All tests passed!"
echo ""
