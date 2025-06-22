import init, { put, IsBlocked, Log } from './pkg/bresenham_lighting_engine.js';

const canvas = document.getElementById('game')
const ctx = canvas.getContext('2d')
const wallsCanvas = document.getElementById('walls')
const wallsCtx = wallsCanvas.getContext('2d', {willReadFrequently: true})

const blackPixel = new ImageData(new Uint8ClampedArray([255,255,255,255]),1)
const transPixel = new ImageData(new Uint8ClampedArray([255,255,255,0]),1)

await init();

console.time('[perf] init')
// wasm.instance.exports.init() - This is now called on start
console.timeEnd('[perf] init')

const size = 60 * 2 + 1
function update(x, y, r){
    console.time('[perf] update')
    const canvasPtr = put(34, r, x, y)
    console.timeEnd('[perf] update')

    console.time('[perf] canvas')
    ctx.clearRect(0, 0, 180, 180)
    // const mem = wasm.instance.exports.memory.buffer - wasm-bindgen abstracts this
    const cells = new Uint8ClampedArray(put.memory.buffer, canvasPtr, size * size * 4)
    const imageData = new ImageData(cells, size, size)
    ctx.putImageData(imageData, x - size/2, y - size/2)

    ctx.globalCompositeOperation = 'destination-over'
    ctx.fillStyle = 'black'
    ctx.fillRect(0, 0, 180, 180)
    console.timeEnd('[perf] canvas')
}

const form = new FormData(document.getElementById('controls'))
update(form.get('x'), form.get('y'), form.get('radius'))

document.getElementById('controls')
    .addEventListener('input', function(ev) {
        const form = new FormData(ev.target.parentElement)
        update(form.get('x'), form.get('y'), form.get('radius'))
    })

function draw(ev){
    ev.preventDefault()
    ev.stopPropagation()
    const width = wallsCanvas.getBoundingClientRect()?.width ?? 450
    if(ev.buttons === 1 || ev.buttons === 2){
        const x = Math.floor(ev.offsetX / width * 180)
        const y = Math.floor(ev.offsetY / width * 180)
        if(x < 0 || x >= 180 || y < 0 || y >= 180) return;

        const pixel = ev.buttons === 1 ? blackPixel : transPixel
        wallsCtx.putImageData(pixel, x, y)

        const form = new FormData(document.getElementById('controls'))
        update(form.get('x'), form.get('y'), form.get('radius'))
    }
    if(ev.buttons === 4){
        const x = Math.floor(ev.offsetX / width * 180)
        const y = Math.floor(ev.offsetY / width * 180)
        if(x < 0 || x >= 180 || y < 0 || y >= 180) return;
        update(x, y, form.get('radius'))
    }
}
wallsCanvas.addEventListener('pointermove', draw)
wallsCanvas.addEventListener('pointerdown', draw)
