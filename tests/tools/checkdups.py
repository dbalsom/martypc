import os
import json
import hashlib
import sys
import gzip
import shutil

def hash_json_object(obj):
    """Hash a JSON object."""
    return hashlib.sha256(json.dumps(obj, sort_keys=True).encode()).hexdigest()

def process_directory(dir_path):
    """Process each JSON file in the specified directory."""
    hash_dict = {}
    total_cycles = 0

    # Get all files in the directory with .gz or .json extension
    files = [f for f in os.listdir(dir_path) if os.path.isfile(os.path.join(dir_path, f)) and (f.endswith('.gz') or f.endswith('.json'))]

    for file in files:
        extracted_file = None

        # If file is a gzipped JSON, decompress it
        if file.endswith('.gz'):
            extracted_file = os.path.join(dir_path, file.replace('.gz', ''))
            with gzip.open(os.path.join(dir_path, file), 'rb') as f_in:
                with open(extracted_file, 'wb') as f_out:
                    shutil.copyfileobj(f_in, f_out)
            file_to_process = extracted_file
        else:
            file_to_process = os.path.join(dir_path, file)

        # Process the JSON file
        with open(file_to_process, 'r') as f:
            try:
                data = json.load(f)
                if not isinstance(data, list):
                    print(f"Warning: File {file} does not contain a JSON array. Skipping...")
                    continue
                
                for idx, obj in enumerate(data):
                    # Increment total_cycles by the length of the 'cycles' key's value (if it exists)
                    total_cycles += len(obj.get('cycles', []))

                    h = hash_json_object(obj)
                    if h in hash_dict:
                        original_file, original_idx = hash_dict[h]
                        print(f"Duplicate found in file {file} at test index {idx}. Original in {original_file} at index {original_idx}.")
                    else:
                        hash_dict[h] = (file, idx)
                
                print(f"File {file} contains {len(data)} tests.")

            except json.JSONDecodeError:
                print(f"Warning: File {file} is not a valid JSON. Skipping...")
        
        # Remove the extracted file after processing if it exists
        if extracted_file and os.path.exists(extracted_file):
            os.remove(extracted_file)

    print(f"Processed {len(files)} files and found {len(hash_dict)} unique tests.")
    print(f"Total number of cycles seen: {total_cycles}")

if __name__ == "__main__":
    if len(sys.argv) != 2:
        print("Usage: python checkdups.py <directory_path>")
        sys.exit(1)

    dir_path = sys.argv[1]
    process_directory(dir_path)