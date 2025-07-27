import { html } from 'https://esm.sh/htm/preact';

export default function TestStep({ 
    id, 
    title, 
    status, 
    className = '', 
    details = '', 
    showButton = false, 
    buttonText = '', 
    onButtonClick = null,
    extraButtons = [],
    children = null 
}) {
    return html`
        <div id="test-${id}" class="test-container ${className}">
            <h3>${title}</h3>
            <p>Status: <span id="${id}-status">${status}</span></p>
            
            ${showButton && html`
                <button onClick=${onButtonClick} disabled=${!onButtonClick}>
                    ${buttonText}
                </button>
            `}
            
            ${extraButtons.map(button => html`
                <button 
                    onClick=${button.onClick} 
                    disabled=${!button.enabled}
                    key=${button.text}
                >
                    ${button.text}
                </button>
            `)}
            
            ${children}
            
            ${details && html`
                <pre id="${id}-details">${details}</pre>
            `}
        </div>
    `;
} 