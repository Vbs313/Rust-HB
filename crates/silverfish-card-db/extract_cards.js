#!/usr/bin/env node
/**
 * 从 C# CardDB_cardIDEnum.cs 提取卡牌数据 → card_data.json
 *
 * 文档注释结构：
 *   /// <summary>
 *   /// <para>{类型} {职业} 费用：N [攻击力：X] [生命值：Y] [耐久度：W]</para>
 *   /// <para>{英文名}</para>
 *   /// <para>{中文名}</para>
 *   /// <para>{英文描述}</para>
 *   /// <para>{中文描述}</para>
 *   /// </summary>
 *   枚举名 = 数值,
 */

const fs = require("fs");
const path = require("path");

const inputFile = path.resolve(
	__dirname,
	"../../Hearthbuddy_backed/Routines/DefaultRoutine/Silverfish/ai/CardDB/CardDB_cardIDEnum.cs",
);
const outputFile = path.resolve(__dirname, "card_data.json");

const content = fs.readFileSync(inputFile, "utf8");
const lines = content.split("\n");
console.log(
	`File: ${path.basename(inputFile)} (${lines.length} lines, ${(content.length / 1024 / 1024).toFixed(2)} MB)`,
);

const cards = [];

for (let i = 0; i < lines.length; i++) {
	const trimmed = lines[i].trim();
	if (!trimmed.startsWith("/// <para>")) continue;

	// 收集连续5个 <para> 行
	const paras = [];
	let j = i;
	while (j < lines.length && paras.length < 5) {
		const t = lines[j].trim();
		if (t.startsWith("/// <para>")) {
			const m = t.match(/<para>(.*?)<\/para>/);
			paras.push(m ? m[1] : "");
		} else if (t.startsWith("/// </summary>")) {
			break;
		}
		j++;
	}
	if (paras.length < 5) {
		i = j;
		continue;
	}

	const [p1, p2, p3, p4, p5] = paras;
	let cardType = "",
		cardClass = "",
		cardCost = 0;
	let attack = 0,
		health = 0,
		armor = 0,
		durability = 0;

	// 格式1（完整）：随从 中立 费用：3 攻击力：1 生命值：4
	let m1 = p1.match(
		/^(.+?) (.+?) 费用：(\d+)(?: 攻击力：(\d+))?(?: 生命值：(\d+))?(?: 护甲值：(\d+))?(?: 耐久度：(\d+))?$/,
	);
	if (m1) {
		cardType = m1[1].trim();
		cardClass = m1[2].trim();
		cardCost = parseInt(m1[3], 10);
		if (m1[4]) attack = parseInt(m1[4], 10);
		if (m1[5]) health = parseInt(m1[5], 10);
		if (m1[6]) armor = parseInt(m1[6], 10);
		if (m1[7]) durability = parseInt(m1[7], 10);
	} else {
		// 格式2：英雄 恶魔猎手 费用：0
		m1 = p1.match(/^(.+?) (.+?) 费用：(\d+)$/);
		if (m1) {
			cardType = m1[1].trim();
			cardClass = m1[2].trim();
			cardCost = parseInt(m1[3], 10);
		} else {
			// 格式3：只有类型和费用
			m1 = p1.match(/^(.+?) 费用：(\d+)$/);
			if (m1) {
				cardType = m1[1].trim();
				cardCost = parseInt(m1[2], 10);
			}
		}
	}

	const nameEN = p2.trim();
	const nameCN = p3.trim();
	const textEN = p4.trim();
	const textCN = p5.trim();

	// 找到枚举名 → 跳过剩余注释行直到枚举行
	while (j < lines.length) {
		const t = lines[j].trim();
		if (t === "/// </summary>" || !t || t === "///") {
			j++;
			continue;
		}
		const enumMatch = t.match(/^(\w+)\s*=\s*(\d+)\s*,/);
		if (enumMatch) {
			const enumName = enumMatch[1];
			const enumValue = parseInt(enumMatch[2], 10);
			if (enumName !== "None") {
				let setId = 0;
				const pfx = enumName.match(/^([A-Z]+[0-9]*)_/);
				if (pfx) {
					const m = {
						CS1: 3,
						CS2: 3,
						NEW1: 3,
						EX1: 3,
						HERO: 3,
						TB: 99,
						GVG: 4,
						BRM: 5,
						TGT: 6,
						LOE: 7,
						KAR: 8,
						UNG: 9,
						ICC: 10,
						LOOT: 11,
						GIL: 12,
						BOT: 13,
						TRL: 14,
						DAL: 15,
						ULD: 16,
						DRG: 17,
						BT: 18,
						SCH: 19,
						DMF: 20,
						BAR: 21,
						STM: 22,
						AV: 23,
						ONY: 23,
						TSC: 24,
						REV: 25,
						TTN: 26,
						RLK: 27,
						MAW: 28,
						TITANS: 28,
						DEEP: 29,
						WW: 30,
						JAM: 31,
						GDB: 32,
						TT: 33,
						NWA: 34,
						SC: 34,
						AIBot: 9999,
						MIS: 9998,
					};
					setId = m[pfx[1]] || 0;
				}
				cards.push({
					id: enumValue,
					name: enumName,
					type: cardType,
					class: cardClass,
					cost: cardCost,
					attack: attack,
					health: health,
					armor: armor,
					durability: durability,
					name_en: nameEN,
					name_cn: nameCN,
					text_en: textEN,
					text_cn: textCN,
					set_id: setId,
				});
			}
			break;
		}
		j++;
	}
	i = j;
}

// 统计
console.log(`Total: ${cards.length}`);
const typeCount = {};
let minionCount = 0,
	spellCount = 0,
	weaponCount = 0;
cards.forEach((c) => {
	typeCount[c.type] = (typeCount[c.type] || 0) + 1;
	if (c.type === "随从") minionCount++;
	if (c.type === "法术") spellCount++;
	if (c.type === "武器") weaponCount++;
});
console.log("\n类型分布 (Top 15):");
Object.entries(typeCount)
	.sort((a, b) => b[1] - a[1])
	.slice(0, 15)
	.forEach(([k, v]) => console.log(`  ${k.padEnd(20)} ${v}`));
console.log(
	`\n随从: ${minionCount}, 法术: ${spellCount}, 武器: ${weaponCount}`,
);
console.log(`有攻击力: ${cards.filter((c) => c.attack > 0).length}`);
console.log(`有生命值: ${cards.filter((c) => c.health > 0).length}`);
console.log(
	`有类型: ${cards.filter((c) => c.type).length}/${cards.length} (${((cards.filter((c) => c.type).length / cards.length) * 100).toFixed(1)}%)`,
);

// 输出
const output = JSON.stringify({
	version: "2026-05-18",
	total: cards.length,
	cards,
});
fs.writeFileSync(outputFile, output);
console.log(
	`\nWritten: ${outputFile} (${(output.length / 1024 / 1024).toFixed(2)} MB)`,
);
