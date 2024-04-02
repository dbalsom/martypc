import os
import zipfile
import argparse

def unzip_files_in_directory(path: str):
    # Check if the provided path exists
    if not os.path.exists(path):
        print(f"Error: Path '{path}' does not exist.")
        return
    
    # Get a list of all files in the directory
    files = os.listdir(path)
    
    # Filter for only .zip files
    zip_files = [f for f in files if f.endswith('.zip')]
    
    if not zip_files:
        print("No .zip files found in the directory.")
        return
    
    # Iterate over each zip file and extract it
    for zip_file in zip_files:
        full_zip_path = os.path.join(path, zip_file)
        with zipfile.ZipFile(full_zip_path, 'r') as zip_ref:
            print(f"Extracting {zip_file} ...")
            zip_ref.extractall(path)
        print(f"Extracted {zip_file} successfully!")

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description='Unzip all the zip files in a specified directory.')
    parser.add_argument('path', type=str, help='Path to the directory containing the zip files.')
    
    args = parser.parse_args()
    unzip_files_in_directory(args.path)