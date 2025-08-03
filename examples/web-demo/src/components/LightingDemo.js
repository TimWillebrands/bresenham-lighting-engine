import { html } from 'https://esm.sh/htm/preact';
import ControlPanel from './ControlPanel.js';
import CanvasContainer from './CanvasContainer.js';
import Instructions from './Instructions.js';
import PerformanceMetrics from './PerformanceMetrics.js';

export default function LightingDemo({ lighting, initTime }) {
    return html`
        <${ControlPanel} 
            lightConfig=${lighting.lightConfig}
            onLightConfigChange=${lighting.updateLightConfig}
            roomsConfigured=${lighting.roomsConfigured}
            onCreateRoomLayout=${lighting.createSimpleRoomLayout}
        />

        <${CanvasContainer}
            canvasRef=${lighting.canvasRef}
            wallsCanvasRef=${lighting.wallsCanvasRef}
            onWallPixelUpdate=${lighting.updateWallPixel}
            onLightMove=${lighting.updateLightConfig}
            updateLighting=${lighting.updateLighting}
        />

        <${Instructions} />

        <${PerformanceMetrics} 
            perfMetrics=${lighting.perfMetrics}
            initTime=${initTime}
        />
    `;
} 