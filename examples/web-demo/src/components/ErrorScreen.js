import { html } from 'https://esm.sh/htm/preact';

export default function ErrorScreen({ error }) {
    return html`
        <div class="error">
            <strong>Error:</strong> ${error.message}
            <details style="margin-top: 12px">
                <summary>Troubleshooting</summary>
                <ul style="margin-top: 8px">
                    <li>
                        Make sure you're using a modern browser that
                        supports WebAssembly
                    </li>
                    <li>Try refreshing the page</li>
                    <li>
                        Check your browser's console for detailed error
                        messages
                    </li>
                    <li>
                        Ensure you're accessing the page over HTTPS or
                        localhost
                    </li>
                </ul>
            </details>
        </div>
    `;
} 