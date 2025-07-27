import { html } from 'https://esm.sh/htm/preact';
import { useState, useEffect } from 'https://esm.sh/preact/hooks';

export default function Footer() {
    const [buildInfo, setBuildInfo] = useState("Loading build info...");

    useEffect(() => {
        // Load build information if available
        fetch("./build-info.json")
            .then((response) => response.json())
            .then((data) => {
                const buildDate = new Date(data.build_time).toLocaleString();
                setBuildInfo(`Built on ${buildDate} (${data.commit_sha.slice(0, 7)})`);
            })
            .catch(() => {
                setBuildInfo("Development build");
            });
    }, []);

    return html`
        <div class="footer">
            <p>
                Built with <strong>Rust 🦀</strong> +
                <strong>WebAssembly 🕸️</strong> +
                <strong>Preact ⚛️</strong>
            </p>
            <p>
                <a
                    href="https://github.com/your-username/bresenham-lighting-engine"
                    target="_blank"
                    rel="noopener"
                >Source Code</a>
                |
                <a
                    href="https://github.com/your-username/bresenham-lighting-engine/issues"
                    target="_blank"
                    rel="noopener"
                >Report Issues</a>
                |
                <a
                    href="https://webassembly.org/"
                    target="_blank"
                    rel="noopener"
                >Learn WebAssembly</a>
            </p>
            <p style="font-size: 0.8rem; opacity: 0.7">
                ${buildInfo}
            </p>
        </div>
    `;
} 