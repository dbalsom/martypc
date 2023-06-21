        
    let wasmInitialized = false;
    import init, { BrowserStatus, start, run } from './marty_pixels_wasm32_player.js';

    // Initialize the WebAssembly module
    async function runWasm() {
        if (!wasmInitialized) {
            console.log('Initializing wasm...');
            await init(); // This is necessary to initialize the WASM module

            window.sharedState = {
                browserStatus: true
            };
            
            // Call the start function or any other exported function
            run(window.browserStatus);
        }
    }
    
    // Attach the runWasm function to the button
    document.getElementById('run-button').addEventListener('click', runWasm);

    function updateCanvasPosition() {
        const windowWidth = window.innerWidth;
        const leftPanelWidth = 340; // 300px width + 20px padding + 20px margin
        const canvasWidth = 768;
        const canvasContainer = document.getElementById('marty-canvas-container');

        const centerPosition = (windowWidth - canvasWidth) / 2;

        if (centerPosition < leftPanelWidth) {
            const gradualLeftMargin = Math.max(leftPanelWidth, centerPosition);
            canvasContainer.style.marginLeft = gradualLeftMargin + 'px';
            canvasContainer.style.marginRight = '20px';
        } else {
            canvasContainer.style.marginLeft = 'auto';
            canvasContainer.style.marginRight = 'auto';
        }
    }

    window.addEventListener('resize', updateCanvasPosition);
    window.addEventListener('blur', function() {
        console.log('Window is no longer in focus, pausing emulator.');
        
        if ( window.sharedState != null ) {
            window.sharedState.browserStatus = false;
        }
    });

    window.addEventListener('focus', function() {
        console.log('Window is now in focus, resuming emulator.');
        
        if ( window.sharedState != null ) {
            window.sharedState.browserStatus = true;
        }
    });
    
    document.addEventListener('DOMContentLoaded', updateCanvasPosition);
    
    document.addEventListener('DOMContentLoaded', function() {
        // Fetch the JSON file
        
        // Get the path, e.g., "/path/foo.html"
        var path = window.location.pathname;

        // Extract the base filename
        var file = path.substring(path.lastIndexOf('/') + 1);

        // Set the src to the same base name with .json extension
        json_file = baseName + '.json';
        
        fetch(json_file)
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
                `;
                
            document.title = `MartyPC Player - ${data.title}`;
            })
            .catch(error => console.error('Error fetching the JSON file:', error));
    });    