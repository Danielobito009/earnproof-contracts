import { createHash } from "node:crypto";

const sha256Hex = (value: string) => createHash("sha256").update(Buffer.from(value, "utf8")).digest("hex");
console.log(sha256Hex("proof:example:1"));
const schema = Buffer.alloc(4);
schema.writeUInt32BE(7);
console.log(schema.toString("hex"));
const expiration = Buffer.alloc(8);
expiration.writeBigUInt64BE(1700000000n);
console.log(expiration.toString("hex"));