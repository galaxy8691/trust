import { Regex } from "regex";

function main(): number {
  let re: Regex = new Regex("\\d+");
  if (re.is_match("abc123")) {
    return 1;
  }
  return 0;
}
