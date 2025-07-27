import { html } from 'https://esm.sh/htm/preact';

export default function ControlPanel({ lightConfig, onLightConfigChange }) {
    const { x, y, radius } = lightConfig;

    const handleInputChange = (key) => (e) => {
        const value = parseInt(e.target.value);
        onLightConfigChange({ [key]: value });
    };

    return html`
        <form class="controls">
            <div class="control-group">
                <label for="x">
                    Light X Position
                    <span class="control-value">${x}</span>
                </label>
                <input
                    type="range"
                    value=${x}
                    min="0"
                    max="180"
                    name="x"
                    id="x"
                    aria-label="Light X Position"
                    onInput=${handleInputChange('x')}
                />
            </div>
            <div class="control-group">
                <label for="y">
                    Light Y Position
                    <span class="control-value">${y}</span>
                </label>
                <input
                    type="range"
                    value=${y}
                    min="0"
                    max="180"
                    name="y"
                    id="y"
                    aria-label="Light Y Position"
                    onInput=${handleInputChange('y')}
                />
            </div>
            <div class="control-group">
                <label for="radius">
                    Light Radius
                    <span class="control-value">${radius}</span>
                </label>
                <input
                    type="range"
                    value=${radius}
                    min="5"
                    max="60"
                    name="radius"
                    id="radius"
                    aria-label="Light Radius"
                    onInput=${handleInputChange('radius')}
                />
            </div>
        </form>
    `;
} 