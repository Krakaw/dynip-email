#!/bin/bash
set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
SERVICE_NAME="dynip-email"
SERVICE_USER="dynip-email"
SERVICE_GROUP="dynip-email"
INSTALL_DIR="/opt/dynip-email"
DATA_DIR="/var/lib/dynip-email"
BINARY_NAME="dynip-email"

echo -e "${BLUE}🚀 Installing DynIP Email Service${NC}"

# Check if running as root
if [ "$EUID" -ne 0 ]; then
    echo -e "${RED}❌ This script must be run as root (use sudo)${NC}"
    exit 1
fi

# Check if systemd is available
if ! command -v systemctl &> /dev/null; then
    echo -e "${RED}❌ systemctl not found. This script requires systemd.${NC}"
    exit 1
fi

# Create service user and group
echo -e "${YELLOW}👤 Creating service user and group...${NC}"
if ! id "$SERVICE_USER" &>/dev/null; then
    useradd --system --no-create-home --shell /bin/false "$SERVICE_USER"
    echo -e "${GREEN}✅ Created user: $SERVICE_USER${NC}"
else
    echo -e "${YELLOW}⚠️  User $SERVICE_USER already exists${NC}"
fi

# Create directories
echo -e "${YELLOW}📁 Creating directories...${NC}"
mkdir -p "$INSTALL_DIR"
mkdir -p "$DATA_DIR"
mkdir -p "$INSTALL_DIR/data"

# Set ownership
chown -R "$SERVICE_USER:$SERVICE_GROUP" "$INSTALL_DIR"
chown -R "$SERVICE_USER:$SERVICE_GROUP" "$DATA_DIR"

# Set permissions
chmod 755 "$INSTALL_DIR"
chmod 755 "$DATA_DIR"
chmod 755 "$INSTALL_DIR/data"

# Copy binary (assuming it's in the current directory or target/release)
if [ -f "./target/release/$BINARY_NAME" ]; then
    echo -e "${YELLOW}📦 Copying binary from target/release...${NC}"
    cp "./target/release/$BINARY_NAME" "$INSTALL_DIR/$BINARY_NAME"
elif [ -f "./$BINARY_NAME" ]; then
    echo -e "${YELLOW}📦 Copying binary from current directory...${NC}"
    cp "./$BINARY_NAME" "$INSTALL_DIR/$BINARY_NAME"
else
    echo -e "${RED}❌ Binary $BINARY_NAME not found. Please build the application first.${NC}"
    echo -e "${YELLOW}💡 Run: cargo build --release${NC}"
    exit 1
fi

# Set binary permissions
chown "$SERVICE_USER:$SERVICE_GROUP" "$INSTALL_DIR/$BINARY_NAME"
chmod +x "$INSTALL_DIR/$BINARY_NAME"

# Copy service file
echo -e "${YELLOW}📋 Installing systemd service...${NC}"
cp "./scripts/dynip-email.service" "/etc/systemd/system/$SERVICE_NAME.service"

# Reload systemd
echo -e "${YELLOW}🔄 Reloading systemd daemon...${NC}"
systemctl daemon-reload

# Enable service
echo -e "${YELLOW}🔧 Enabling service...${NC}"
systemctl enable "$SERVICE_NAME"

echo -e "${GREEN}✅ Installation complete!${NC}"
echo ""
echo -e "${BLUE}📋 Next steps:${NC}"
echo -e "1. Edit configuration: ${YELLOW}sudo nano /etc/systemd/system/$SERVICE_NAME.service${NC}"
echo -e "2. Start the service: ${YELLOW}sudo systemctl start $SERVICE_NAME${NC}"
echo -e "3. Check status: ${YELLOW}sudo systemctl status $SERVICE_NAME${NC}"
echo -e "4. View logs: ${YELLOW}sudo journalctl -u $SERVICE_NAME -f${NC}"
echo ""
echo -e "${BLUE}🔧 Configuration notes:${NC}"
echo -e "• Default database location: ${YELLOW}$DATA_DIR/emails.db${NC}"
echo -e "• Default API port: ${YELLOW}3000${NC}"
echo -e "• Default SMTP port: ${YELLOW}2525${NC}"
echo -e "• To enable SSL, uncomment SSL environment variables in the service file${NC}"
echo -e "• Service runs as user: ${YELLOW}$SERVICE_USER${NC}"
echo ""
echo -e "${GREEN}🎉 DynIP Email service is ready to use!${NC}"
