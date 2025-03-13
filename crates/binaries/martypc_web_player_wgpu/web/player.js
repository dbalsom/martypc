        
    let wasmInitialized = false;
    import init, { start, run } from './marty_pixels_wasm32_player.js';

    // Initialize the WebAssembly module
    async function runWasm() {
        if (!wasmInitialized) {
            console.log('Initializing wasm...');
            await init(); // This is necessary to initialize the WASM module
            
            // Call the start function or any other exported function
            wasmInitialized = true;
            run(window.sharedState.cfg);
        }
    }
    
    // Attach the runWasm function to the button
    document.getElementById('run-button').addEventListener('click', runWasm);

    function updateCanvasPosition() {

        const canvasContainer = document.getElementById('marty-canvas-container');

        if (window.innerWidth < 1170) {
            canvasContainer.style.marginLeft = 'auto';
            canvasContainer.style.marginRight = 'auto';
            canvasContainer.style.marginBottom = '20px';
            return
        }

        /*
        const windowWidth = window.innerWidth;
        const leftPanelWidth = 340; // 300px width + 20px padding + 20px margin
        const canvasWidth = 768;

        const centerPosition = (windowWidth - canvasWidth) / 2;

        if (centerPosition < leftPanelWidth) {
            const gradualLeftMargin = Math.max(leftPanelWidth, centerPosition);
            canvasContainer.style.marginLeft = gradualLeftMargin + 'px';
            canvasContainer.style.marginRight = '20px';
        } else {
            canvasContainer.style.marginLeft = 'auto';
            canvasContainer.style.marginRight = 'auto';
        }
        */
    }

    function sanitizeTitle(title) {
        // Remove all characters that are not alphanumeric
        return title.replace(/[^a-zA-Z0-9]/g, '');
    }

    window.addEventListener('resize', updateCanvasPosition);
    window.addEventListener('blur', function() {
        console.log('Window is no longer in focus, pausing emulator.');
        
        if ( window.sharedState != null ) {
            window.sharedState.browserFocus = false;
        }
    });

    window.addEventListener('focus', function() {
        console.log('Window is now in focus, resuming emulator.');
        
        if ( window.sharedState != null ) {
            window.sharedState.browserFocus = true;
        }
    });
    
    document.addEventListener('DOMContentLoaded', updateCanvasPosition);
    
    document.addEventListener('DOMContentLoaded', function() {
        
        window.sharedState = {
            browserFocus: true,
            cfg: ""
        };        
        
        // Fetch the JSON file

        // Get the URL query parameters
        var params = new URLSearchParams(window.location.search);

        // Get the title parameter from the URL, e.g., "area5150" from "?title=area5150"
        var title = sanitizeTitle(params.get('title'));

        // If the title parameter exists, use it to create the JSON file name
        if (title) {
            var jsonFile = './cfg/' + title + '.json';
        
            fetch(jsonFile)
                .then(response => response.json())
                .then(data => {
                    // Select the table by its ID and populate it with data from the JSON file
                    document.querySelector('#title-info').innerHTML = `
                        <tr>
                            <td>Title:</td>
                            <td>${data.title}</td>
                        </tr>
                        <tr>
                            <td>Developer:</td>
                            <td>${data.developer}</td>
                        </tr>                    
                        <tr>
                            <td>Platform:</td>
                            <td>${data.platform}</td>
                        </tr>
                        <tr>
                            <td>Year:</td>
                            <td>${data.year}</td>
                        </tr>
                        <tr>
                            <td>Link:</td>
                            <td><a href="${data.link}" target="_blank">More Info</a></td>
                        </tr>       
                        <tr>
                            <td class="notes-header">Instructions:</td>
                            <td>${data.instructions}</a></td>
                        </tr>                                    
                        <tr>
                            <td class="notes-header">Notes:</td>
                            <td><div class="title-notes">${data.notes}</div></td>
                        </tr>                             
                    `;
                
                    window.sharedState.cfg = data.cfg;
                    document.title = `MartyPC Player - ${data.title}`;
                })
                .catch(error => console.error('Error fetching the JSON file:', error));
        } else {
            console.error('Title parameter is missing in the URL.');
        }

    });    