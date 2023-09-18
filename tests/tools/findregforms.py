import json
import gzip
import re
import sys

def load_json(filename):
    """Load a json file. If has .gz extension, decompress it first."""
    if filename.endswith('.gz'):
        with gzip.open(filename, 'rt') as f:
            return json.load(f)
    else:
        with open(filename, 'r') as f:
            return json.load(f)

def matches_pattern(instruction):
    """Check if the instruction matches 'mnemonic r, r' pattern."""
    registers = ['ax', 'ah', 'al', 'bx', 'bh', 'bl', 'cx', 'bh', 'cl', 
                 'dx', 'dh', 'dl', 'ss', 'es', 'cs', 'ds', 'sp', 'bp', 
                 'si', 'di']
    pattern = r'(?<![\w:[])(' + '|'.join(registers) + r')(?![\w])'
    search_results = list(re.finditer(pattern, instruction))
    
    # If two registers are found, and no other match exists, we can consider it a valid match
    if len(search_results) == 2:
        mnemonic = instruction.split()[0]
        return f"{mnemonic} {search_results[0].group()}, {search_results[1].group()}" == instruction.strip()
    return False

def main():
    if len(sys.argv) != 2:
        print("Usage: script_name.py <path_to_file>")
        sys.exit(1)

    filename = sys.argv[1]
    json_data = load_json(filename)

    for item in json_data:
        if 'name' in item and matches_pattern(item['name']):
            print(f"Match found: {item['name']}")

if __name__ == "__main__":
    main()





