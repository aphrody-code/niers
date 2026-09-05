import { google } from "googleapis";
import * as dotenv from "dotenv";
import { resolve, join } from "node:path";
import * as fs from "node:fs";

dotenv.config({ path: resolve(import.meta.dirname, "../.env") });
dotenv.config({ path: resolve(import.meta.dirname, "../../.env") });

const DOC_ID = "1AGlA8GADu_KBaX-Rr5p_9BsQqe8bTXtzGKLBGOjsdJY";
// Paramétrable : le chemin d'origine désignait le dépôt d'une machine précise.
const DOCS_DIR =
	process.env.AZALEE_SCREENSHOTS_DIR ?? resolve(import.meta.dirname, "../../../docs/tools/screenshots");
const SCRIPT_PATH = "/home/ubuntu/.gemini/antigravity-cli/brain/55f253a8-fe2a-43f0-a9de-53e6b7a11964/youtube_video_script.md";

const SECTION_MAP: Record<string, string[]> = {
	"1. INTRODUCTION": [],
	"2. L'ACCUEIL": ["accueil.png"],
	"3. PORTAIL INAZUMA ELEVEN CROSS": ["cross.png"],
	"4. LE CODEX DES JOUEURS": ["chara.png", "chara-mark.png"],
	"5. LA BASE DES TECHNIQUES HISSATSU": ["skill.png", "skill-mur.png"],
	"6. INVOCATIONS, KESHINS ET AURAS": ["aura.png", "aura-keshin.png"],
	"7. L'INVENTAIRE DES OBJETS": ["item.png", "item-guts.png"],
	"8. LES TALENTS PASSIFS": ["passive.png"],
	"9. LES SUPER TACTIQUES": ["tactic.png", "tactic-shinano.png"],
	"10. LES BOUTIQUES": ["boutique.png", "boutique-esprits.png"],
	"11. L'EXPLORATEUR DE DROPS": ["drops.png"],
	"12. LA GALERIE": ["gallery.png"],
	"13. LE LECTEUR DE SAUVEGARDES": ["save.png"],
	"14. L'EXPLORATEUR DE FICHIERS CPK": ["cpk.png"],
	"15. LE VISUALISEUR DE MENUS": ["menu.png"],
	"16. LE CODEX DES QUÊTES": ["quete.png", "quete-classroom.png"],
	"17. LE SYSTÈME DE SUCCÈS": ["succes.png", "succes-sable.png"],
	"18. L'ANNUAIRE DES STADES": ["stade.png", "stade-index60.png"],
	"19. LES ENTRAÎNEURS": ["entraineur.png", "entraineur-silvia.png"],
	"20. LE COMPARATEUR": ["compare.png"],
	"21. LE GÉNÉRATEUR D'ÉQUIPE ALÉATOIRE": ["random-team.png"],
	"22. LE TRADUCTEUR": ["translator.png"],
	"23. LE CONSTRUCTEUR D'ÉQUIPE": ["my-team.png"],
	"24. LE BLOG D'ACTUALITÉS": ["news.png"],
	"25. LE TABLEAU DE BORD": ["dashboard.png"],
	"26. LES CAPSULES & COSTUMES GACHA": ["capsule.png", "capsule-detail.png"],
	"27. L'ANNUAIRE DES ÉQUIPES": ["equipe.png", "equipe-detail.png"],
	"28. LES TAUX D'INVOCATION": ["invocation.png"],
	"29. LES MODÈLES 3D DES ESPRITS GUERRIERS": ["keshin-models.png"],
	"30. LES MODÈLES 3D DES TECHNIQUES": ["modeles.png"],
	"31. LES NOTES DE MISE À JOUR": ["patch-notes.png", "patch-notes-detail.png"],
	"32. CONCLUSION": []
};

function parseDirtyJson(str: string) {
	let inString = false;
	let result = "";
	for (let i = 0; i < str.length; i++) {
		const char = str[i];
		if (char === '"' && (i === 0 || str[i - 1] !== "\\")) {
			inString = !inString;
			result += char;
		} else if (char === "\n" && inString) {
			result += "\\n";
		} else {
			result += char;
		}
	}
	return JSON.parse(result);
}

function getGoogleAuth() {
	let credentials: any = null;

	if (process.env.GOOGLE_SERVICE_ACCOUNT) {
		try {
			credentials = parseDirtyJson(process.env.GOOGLE_SERVICE_ACCOUNT);
		} catch (e: any) {
			console.error("Failed to parse GOOGLE_SERVICE_ACCOUNT:", e.message);
		}
	}

	if (!credentials && process.env.GCP_ADMIN_SA_KEY) {
		try {
			credentials = parseDirtyJson(process.env.GCP_ADMIN_SA_KEY);
		} catch (e: any) {
			console.error("Failed to parse GCP_ADMIN_SA_KEY:", e.message);
		}
	}

	if (!credentials) {
		throw new Error("No service account credentials could be successfully parsed");
	}

	return new google.auth.JWT({
		email: credentials.client_email,
		key: credentials.private_key,
		scopes: [
			"https://www.googleapis.com/auth/documents"
		]
	});
}

async function uploadToTmpFiles(filePath: string, retries = 3): Promise<string> {
	const file = Bun.file(filePath);
	const formData = new FormData();
	formData.append("file", file);
	formData.append("expire", "86400"); // 24 hours

	for (let attempt = 1; attempt <= retries; attempt++) {
		const controller = new AbortController();
		const timeoutId = setTimeout(() => controller.abort(), 15000); // 15s timeout

		try {
			const response = await fetch("https://tmpfiles.org/api/v1/upload", {
				method: "POST",
				body: formData,
				signal: controller.signal
			});

			clearTimeout(timeoutId);

			if (!response.ok) {
				throw new Error(`Status ${response.status}`);
			}

			const json = await response.json() as any;
			if (json.status !== "success" || !json.data?.url) {
				throw new Error(`Response status: ${json.status}`);
			}

			const viewUrl = json.data.url;
			return viewUrl.replace("https://tmpfiles.org/", "https://tmpfiles.org/dl/");
		} catch (error: any) {
			clearTimeout(timeoutId);
			console.warn(`Attempt ${attempt} failed for ${filePath.split("/").pop()}: ${error.message || error}`);
			if (attempt === retries) {
				throw error;
			}
			// Wait 2 seconds before retry
			await new Promise(r => setTimeout(r, 2000));
		}
	}
	throw new Error("Failed after all retries");
}

async function main() {
	const auth = getGoogleAuth();
	const docs = google.docs({ version: "v1", auth });

	// List files in the local screenshots directory
	const localFiles = fs.readdirSync(DOCS_DIR).filter(f => f.endsWith(".png"));
	console.log(`Found ${localFiles.length} local screenshots in ${DOCS_DIR}.`);

	// Upload each file and get the direct download link
	const fileLinksMap: Record<string, string> = {};
	for (const filename of localFiles) {
		console.log(`Uploading ${filename} to tmpfiles.org...`);
		const filePath = join(DOCS_DIR, filename);
		try {
			const directUrl = await uploadToTmpFiles(filePath);
			fileLinksMap[filename] = directUrl;
			console.log(`Uploaded ${filename} -> URL: ${directUrl}`);
		} catch (error) {
			console.error(`Error uploading ${filename}:`, error);
		}
	}

	console.log("All screenshots uploaded successfully!");

	// Read and parse the YouTube script
	console.log(`Reading script from ${SCRIPT_PATH}...`);
	const scriptContent = fs.readFileSync(SCRIPT_PATH, "utf-8");

	// Split by sections using regex that matches both ## and ### with CRLF support
	const rawSections = scriptContent.split(/\r?\n##+ /);
	const sections: { heading: string; body: string; screenshots: string[] }[] = [];

	// The first element is the introduction before the first heading
	const introText = rawSections[0].trim();
	sections.push({
		heading: "",
		body: introText,
		screenshots: []
	});

	for (let i = 1; i < rawSections.length; i++) {
		const rawSection = rawSections[i];
		const lines = rawSection.split("\n");
		const headingLine = lines[0].trim();
		const heading = "## " + headingLine;
		const body = lines.slice(1).join("\n").trim();

		// Match heading to screenshots
		let matchedScreenshots: string[] = [];
		const normalizedHeading = headingLine.toUpperCase();
		for (const [key, files] of Object.entries(SECTION_MAP)) {
			if (normalizedHeading.includes(key.toUpperCase())) {
				matchedScreenshots = files;
				break;
			}
		}

		sections.push({
			heading,
			body,
			screenshots: matchedScreenshots
		});
	}

	console.log(`Parsed ${sections.length} sections from the script.`);

	// Get current document end index to clear it
	console.log(`Fetching Google Doc ${DOC_ID} structure...`);
	const doc = await docs.documents.get({ documentId: DOC_ID });
	const endIndex = doc.data.body?.content?.slice(-1)[0]?.endIndex || 2;

	console.log(`Document endIndex: ${endIndex}. Building batch update requests...`);
	const requests: any[] = [];

	// 1. Clear the document if it contains text
	if (endIndex > 2) {
		requests.push({
			deleteContentRange: {
				range: {
					startIndex: 1,
					endIndex: endIndex - 1
				}
			}
		});
	}

	// Helper to track length (starts at 1 after clearing)
	let currentLen = 1;

	for (const section of sections) {
		// Insert Heading
		if (section.heading) {
			const headingText = section.heading + "\n";
			requests.push({
				insertText: {
					text: headingText,
					endOfSegmentLocation: {}
				}
			});
			// Format as Heading 2
			requests.push({
				updateParagraphStyle: {
					paragraphStyle: {
						namedStyleType: "HEADING_2",
						spaceAbove: { magnitude: 12, unit: "PT" },
						spaceBelow: { magnitude: 6, unit: "PT" }
					},
					range: {
						startIndex: currentLen,
						endIndex: currentLen + headingText.length
					},
					fields: "namedStyleType,spaceAbove,spaceBelow"
				}
			});
			currentLen += headingText.length;
		}

		// Insert Body
		if (section.body) {
			const bodyText = section.body + "\n\n";
			requests.push({
				insertText: {
					text: bodyText,
					endOfSegmentLocation: {}
				}
			});
			// Format body text styling (Normal text)
			requests.push({
				updateParagraphStyle: {
					paragraphStyle: {
						namedStyleType: "NORMAL_TEXT"
					},
					range: {
						startIndex: currentLen,
						endIndex: currentLen + bodyText.length
					},
					fields: "namedStyleType"
				}
			});
			currentLen += bodyText.length;
		}

		// Insert matched screenshots
		for (const filename of section.screenshots) {
			const directUrl = fileLinksMap[filename];
			if (directUrl) {
				console.log(`Adding image request for ${filename} to section...`);
				requests.push({
					insertInlineImage: {
						uri: directUrl,
						endOfSegmentLocation: {}
					}
				});
				currentLen += 1;

				// Add a caption/spacing newline
				const captionText = `\n[Capture d'écran: ${filename}]\n\n`;
				requests.push({
					insertText: {
						text: captionText,
						endOfSegmentLocation: {}
					}
				});
				// Style the caption as italic and small
				requests.push({
					updateTextStyle: {
						textStyle: {
							italic: true,
							fontSize: { magnitude: 10, unit: "PT" }
						},
						range: {
							startIndex: currentLen,
							endIndex: currentLen + captionText.length
						},
						fields: "italic,fontSize"
					}
				});
				currentLen += captionText.length;
			} else {
				console.warn(`Warning: Screenshot ${filename} was not uploaded or found.`);
			}
		}
	}

	console.log(`Sending batchUpdate to Google Docs with ${requests.length} requests...`);
	const updateResponse = await docs.documents.batchUpdate({
		documentId: DOC_ID,
		requestBody: {
			requests: requests
		}
	});

	console.log("Google Doc updated successfully!", updateResponse.status);
}

main().catch(console.error);
