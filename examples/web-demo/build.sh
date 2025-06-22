#!/bin/bash

# Bresenham Lighting Engine - Development Build Script
#
# This script simplifies the development workflow by:
# - Building the WASM module
# - Copying it to the correct location
# - Providing clear status updates

set -e  # Exit on any error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Directories
PROJECT_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
DEMO_ROOT="$(cd "$(dirname "$0")" && pwd)"
PKG_DIR="$DEMO_ROOT/pkg"

echo -e "${BLUE}🚀 Bresenham Lighting Engine - Build Script${NC}"
echo "=================================================="
echo "📁 Project Root: $PROJECT_ROOT"
echo "📁 Demo Root: $DEMO_ROOT"
echo "📁 Package Dir: $PKG_DIR"
echo ""

# Check prerequisites
echo -e "${YELLOW}🔍 Checking prerequisites...${NC}"

if ! command -v wasm-pack &> /dev/null; then
    echo -e "${RED}❌ wasm-pack is not installed${NC}"
    echo "📥 Install it with: cargo install wasm-pack"
    exit 1
fi

if ! command -v cargo &> /dev/null; then
    echo -e "${RED}❌ cargo is not installed${NC}"
    echo "📥 Install Rust from: https://rustup.rs/"
    exit 1
fi

if [ ! -f "$PROJECT_ROOT/Cargo.toml" ]; then
    echo -e "${RED}❌ Cargo.toml not found in project root${NC}"
    exit 1
fi

echo -e "${GREEN}✅ Prerequisites OK${NC}"
echo ""

# Build WASM module
echo -e "${YELLOW}🔨 Building WASM module...${NC}"
echo "⏱️  This may take a moment..."

cd "$PROJECT_ROOT"

if wasm-pack build \
    --target web \
    --out-dir "$PKG_DIR" \
    --release; then
    echo -e "${GREEN}✅ WASM build completed successfully${NC}"
else
    echo -e "${RED}❌ WASM build failed${NC}"
    exit 1
fi

# Verify the build
echo ""
echo -e "${YELLOW}🔍 Verifying build output...${NC}"

REQUIRED_FILES=(
    "bresenham_lighting_engine.js"
    "bresenham_lighting_engine_bg.wasm"
    "bresenham_lighting_engine.d.ts"
    "package.json"
)

for file in "${REQUIRED_FILES[@]}"; do
    if [ -f "$PKG_DIR/$file" ]; then
        echo -e "✅ $file"
    else
        echo -e "${RED}❌ Missing: $file${NC}"
        exit 1
    fi
done

# Show package info
echo ""
echo -e "${YELLOW}📊 Build Statistics:${NC}"
WASM_SIZE=$(du -h "$PKG_DIR/bresenham_lighting_engine_bg.wasm" | cut -f1)
JS_SIZE=$(du -h "$PKG_DIR/bresenham_lighting_engine.js" | cut -f1)
echo "📦 WASM size: $WASM_SIZE"
echo "📦 JS size: $JS_SIZE"

# Success message
echo ""
echo -e "${GREEN}🎉 Build completed successfully!${NC}"
echo "📍 Files are ready in: $PKG_DIR"
echo ""
echo -e "${BLUE}Next steps:${NC}"
echo "🌐 Start dev server: bun run dev"
echo "🚀 Or run manually: bun run server.js --hot"
echo "🌍 Then open: http://localhost:3000"
