# addhash.py
# Utility to add a 'test_hash' key to each test in a JSON test suite file.
# Supports plain JSON and gzipped JSON test files.

import os
import json
import hashlib
import gzip
import shutil
import sys

def hash_json_object(obj):
    """Generate a hash for a JSON object."""
    obj_str = json.dumps(obj, sort_keys=True).encode()
    return hashlib.sha256(obj_str).hexdigest()

def process_directory(dir_path):
    """Process each JSON or gzipped JSON file in the specified directory."""
    
    # Get all files in the directory with .gz or .json extension
    files = [f for f in os.listdir(dir_path) if os.path.isfile(os.path.join(dir_path, f)) and (f.endswith('.gz') or f.endswith('.json'))]

    for file in files:
        extracted_file = None

        # If file is a gzipped JSON, decompress it
        if file.endswith('.gz'):
            extracted_file = os.path.join(dir_path, file.replace('.gz', ''))

            with gzip.open(os.path.join(dir_path, file), 'rb') as file_in:
                with open(extracted_file, 'wb') as file_out:
                    shutil.copyfileobj(file_in, file_out)
            
            file_to_process = extracted_file
        else:
            file_to_process = os.path.join(dir_path, file)

        # Process the JSON file
        with open(file_to_process, 'r') as f:
            try:
                test_data = json.load(f)
                if not isinstance(test_data, list):
                    print(f"Warning: File {file} does not contain a JSON array. Skipping...")
                    continue
                
                # Add 'test_hash' key to each test object
                for idx, test_obj in enumerate(test_data):
                    test_obj['test_hash'] = hash_json_object(test_obj)
                    test_obj['test_num'] = idx

                # Save the updated data back to the file
                with open(file_to_process, 'w') as f_out:
                    json.dump(test_data, f_out, indent=4)
                    print(f"Processed file {file}.")
                
            except json.JSONDecodeError:
                print(f"Warning: File {file} is not a valid JSON. Skipping...")
        
        # If the file was gzipped originally, compress the updated file again.
        if file.endswith('.gz'):
            with open(file_to_process, 'rb') as f_in:
                with gzip.open(os.path.join(dir_path, file), 'wb') as f_out:
                    shutil.copyfileobj(f_in, f_out)

            os.remove(extracted_file)

    print(f"Processed {len(files)} files.")

if __name__ == "__main__":
    if len(sys.argv) != 2:
        print("Usage: addhash.py <test_directory_path>")
        sys.exit(1)

    process_directory(sys.argv[1])