import { html } from 'https://esm.sh/htm/preact';
import { useEffect } from 'https://esm.sh/preact/hooks';

export default function CanvasContainer({ 
    canvasRef, 
    wallsCanvasRef, 
    onWallPixelUpdate, 
    onLightMove,
    updateLighting
}) {
    
    useEffect(() => {
        const wallsCanvas = wallsCanvasRef.current;
        if (!wallsCanvas) return;

        function drawWall(ev) {
            ev.preventDefault();
            ev.stopPropagation();

            const rect = wallsCanvas.getBoundingClientRect();
            const scaleX = 180 / rect.width;
            const scaleY = 180 / rect.height;

            if (ev.buttons === 1 || ev.buttons === 2) {
                const x = Math.floor(ev.offsetX * scaleX);
                const y = Math.floor(ev.offsetY * scaleY);

                if (x < 0 || x >= 180 || y < 0 || y >= 180) return;

                const isErasing = ev.buttons === 2;
                const ctx = wallsCanvas.getContext('2d');
                const color = isErasing ? "rgba(0,0,0,0)" : "rgba(255,255,255,255)";

                // Draw a small brush (3x3 pixels for better visibility)
                ctx.fillStyle = color;
                for (let dx = -1; dx <= 1; dx++) {
                    for (let dy = -1; dy <= 1; dy++) {
                        const brushX = x + dx;
                        const brushY = y + dy;
                        if (brushX >= 0 && brushX < 180 && brushY >= 0 && brushY < 180) {
                            ctx.fillRect(brushX, brushY, 1, 1);
                            onWallPixelUpdate(brushX, brushY, !isErasing);
                        }
                    }
                }
                            }

                // Update lighting after drawing walls
                updateLighting();

            if (ev.buttons === 4) {
                // Middle mouse button - move light
                const x = Math.floor(ev.offsetX * scaleX);
                const y = Math.floor(ev.offsetY * scaleY);

                if (x < 0 || x >= 180 || y < 0 || y >= 180) return;

                // Update light position
                onLightMove({ x, y });
            }
        }

        // Enhanced pointer events for better touch support
        wallsCanvas.addEventListener("pointermove", drawWall);
        wallsCanvas.addEventListener("pointerdown", drawWall);
        wallsCanvas.addEventListener("touchstart", (e) => e.preventDefault());
        wallsCanvas.addEventListener("touchmove", (e) => e.preventDefault());

        // Prevent context menu on right click
        wallsCanvas.addEventListener("contextmenu", (e) => e.preventDefault());

        return () => {
            if (wallsCanvas) {
                wallsCanvas.removeEventListener("pointermove", drawWall);
                wallsCanvas.removeEventListener("pointerdown", drawWall);
                wallsCanvas.removeEventListener("touchstart", (e) => e.preventDefault());
                wallsCanvas.removeEventListener("touchmove", (e) => e.preventDefault());
                wallsCanvas.removeEventListener("contextmenu", (e) => e.preventDefault());
            }
        };
    }, [wallsCanvasRef, onWallPixelUpdate, onLightMove]);

    return html`
        <div class="canvas-container">
            <canvas
                ref=${canvasRef}
                width="180"
                height="180"
                aria-label="Lighting visualization"
            ></canvas>
            <canvas
                ref=${wallsCanvasRef}
                width="180"
                height="180"
                onContextMenu=${(e) => e.preventDefault()}
                aria-label="Wall drawing canvas"
            ></canvas>
        </div>
    `;
} 