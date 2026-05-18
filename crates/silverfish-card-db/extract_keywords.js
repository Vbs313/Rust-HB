#!/usr/bin/env node
/**
 * 从 CardDefs.xml 提取卡牌关键字标志 → 写入 card_data.json
 * 按 name (card ID) 匹配
 */

const fs = require("fs");
const path = require("path");

const xmlFile = path.resolve(
	__dirname,
	"../../Hearthbuddy_backed/Routines/DefaultRoutine/Silverfish/data/CardDefs.xml",
);
const jsonFile = path.resolve(__dirname, "card_data.json");

// enumID → 关键字名
const KW = {
	190: "has_taunt",
	194: "has_divine_shield",
	197: "has_charge",
	791: "has_rush",
	191: "has_stealth",
	189: "has_windfury",
	363: "has_poisonous",
	685: "has_lifesteal",
	1085: "has_reborn",
	1211: "has_elusive",
	240: "has_immune",
	277: "has_mega_windfury",
};

console.log("Reading CardDefs.xml...");
const xml = fs.readFileSync(xmlFile, "utf8");
const kwMap = {}; // cardID -> { kw: true, ... }
let entities = 0;

const entityRe = /<Entity\s+CardID="([^"]+)"[^>]*>([\s\S]*?)<\/Entity>/g;
let m;
while ((m = entityRe.exec(xml))) {
	entities++;
	const cardId = m[1];
	const body = m[2];
	const kws = {};
	const tagRe = /<Tag\s+enumID="(\d+)"[^>]*\/>/g;
	let t;
	while ((t = tagRe.exec(body))) {
		if (KW[t[1]]) kws[KW[t[1]]] = true;
	}
	if (Object.keys(kws).length > 0) kwMap[cardId] = kws;
}

console.log(
	`  ${entities} entities, ${Object.keys(kwMap).length} with keywords`,
);

// 读 card_data.json
console.log("\nReading card_data.json...");
const json = JSON.parse(fs.readFileSync(jsonFile, "utf8"));
console.log(`  ${json.cards.length} cards`);

// 合并关键字
const kwCounts = {};
let matched = 0;
for (const card of json.cards) {
	const kws = kwMap[card.name];
	if (kws) {
		for (const [k, v] of Object.entries(kws)) {
			card[k] = v;
			kwCounts[k] = (kwCounts[k] || 0) + 1;
		}
		matched++;
	}
}

// 没有显式 true 的字段默认为 false (for Rust serde)
// 但当前 JSON 不需要 false 字段，后续 build.rs 会处理
// 只是 true 的字段会被添加

console.log(`  Matched ${matched}/${json.cards.length} cards`);
console.log("\nKeyword distribution:");
for (const [kw, n] of Object.entries(kwCounts).sort((a, b) => b[1] - a[1])) {
	console.log(`  ${kw}: ${n}`);
}

// 写入 (暂存为新文件，避免覆盖原 card_data.json)
const outFile = path.resolve(__dirname, "card_data_keywords.json");
fs.writeFileSync(outFile, JSON.stringify(json, null, 2));
console.log(`\nWritten to ${outFile}`);
console.log("Done. Update build.rs to use card_data_keywords.json");
