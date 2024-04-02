import os
import sys
import requests
from bs4 import BeautifulSoup

def download_zip_files(url, destination_path):
    try:
        # Check if destination path exists
        if not os.path.exists(destination_path):
            os.makedirs(destination_path)

        # Fetch the HTML content of the URL
        response = requests.get(url)
        response.raise_for_status()  # Raise error if not a successful request

        # Parse the HTML content using BeautifulSoup
        soup = BeautifulSoup(response.text, 'html.parser')

        # Extract all <a> tags from the HTML
        for link in soup.find_all('a', href=True):
            href = link['href']

            # Check if the href ends with ".zip"
            if href.endswith('.zip'):
                zip_url = href

                # Handle relative links
                if not zip_url.startswith(('http://', 'https://')):
                    # Use urljoin to handle relative URLs correctly
                    from urllib.parse import urljoin
                    zip_url = urljoin(url, zip_url)

                local_filename = os.path.join(destination_path, os.path.basename(zip_url))

                # Check if file already exists in destination path
                if os.path.exists(local_filename):
                    print(f"{local_filename} already exists. Skipping download.")
                    continue

                print(f"Downloading {zip_url}...")
                zip_response = requests.get(zip_url, stream=True)
                zip_response.raise_for_status()

                # Save the content to the specified destination path
                with open(local_filename, 'wb') as zip_file:
                    for chunk in zip_response.iter_content(chunk_size=8192):
                        zip_file.write(chunk)
                print(f"Saved to {local_filename}")

    except requests.RequestException as e:
        print(f"Error: {e}")

if __name__ == "__main__":
    if len(sys.argv) != 3:
        print("Usage: python get_remote.py <URL> <destination_path>")
        sys.exit(1)
    
    url_to_fetch = sys.argv[1]
    destination = sys.argv[2]

    download_zip_files(url_to_fetch, destination)