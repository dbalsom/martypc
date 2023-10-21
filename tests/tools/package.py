import os
import gzip
import shutil
import argparse

def gzip_file(filename):
    """GZIP a given file."""
    with open(filename, 'rb') as f_in:
        with gzip.open(filename + '.gz', 'wb') as f_out:
            shutil.copyfileobj(f_in, f_out)
    os.remove(filename)

def starts_with_hex(filename, start_hex, end_hex):
    """Check if the filename starts with a hexadecimal within the given range."""
    if len(filename) < 2:
        return False

    prefix = filename[:2].upper()
    if start_hex <= prefix <= end_hex:
        try:
            int(prefix, 16)
            return True
        except ValueError:
            return False
    return False

def find_and_gzip_json_files(path, start_hex, end_hex):
    """Find JSON files based on the given criteria and GZIP them."""
    for filename in os.listdir(path):
        if not filename.lower().endswith('.json'):
            continue
        
        if starts_with_hex(filename, start_hex, end_hex):
            filepath = os.path.join(path, filename)
            gzip_file(filepath)

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="GZIP JSON files based on filename criteria.")
    parser.add_argument('--path', type=str, help="Path to the directory containing JSON files.")
    parser.add_argument('--start', type=str, help="Starting hexadecimal (00-FF).")
    parser.add_argument('--end', type=str, help="Ending hexadecimal (00-FF).")

    args = parser.parse_args()
    
    start_int = int(args.start, 16)
    end_int = int(args.end, 16)
    
    # Verify that provided hexadecimals are valid
    try:
        if 0x00 <= start_int <= 0xFF and 0x00 <= end_int <= 0xFF and start_int <= end_int:
            find_and_gzip_json_files(args.path, args.start.upper(), args.end.upper())
        else:
            print("Invalid start or end hexadecimal. Please provide hexadecimals in the range 00-FF and ensure start <= end.")
    except ValueError:
        print("Invalid hexadecimal input provided.") 