"use server";

import { google } from "googleapis";
import { revalidatePath } from "next/cache";
import { getServerSession } from "@/lib/auth-helpers";
import { isAdminRole } from "@/lib/auth-roles";
import { convertGoogleDocToHtml } from "@/lib/google-docs";
import { createAdminClient } from "@/lib/supabase/admin";
import { createClient } from "@/lib/supabase/server";

const DRIVE_FOLDER_ID = "19UJsSXVRV0sEd67JCwHSu9Atm3H_5D6K";

function getGoogleAuth() {
	// Le compte de service vient de l'environnement, et de nulle part ailleurs.
	//
	// Une premiere branche lisait `google.json` a cote du processus (« dev/legacy ») avant de
	// consulter la variable. En serverless ce fichier n'existe pas, la branche etait donc morte
	// — mais elle restait une porte : un fichier de cle depose par megarde a la racine aurait
	// silencieusement pris le pas sur la configuration reelle. Une source de secrets doit etre
	// unique, et lisible dans la configuration du deploiement.
	if (process.env.GOOGLE_SERVICE_ACCOUNT) {
		try {
			const credentials = JSON.parse(process.env.GOOGLE_SERVICE_ACCOUNT);
			return new google.auth.GoogleAuth({
				credentials,
				scopes: [
					"https://www.googleapis.com/auth/documents.readonly",
					"https://www.googleapis.com/auth/drive",
				],
			});
		} catch (error) {
			console.error("Invalid GOOGLE_SERVICE_ACCOUNT JSON", error);
		}
	}

	// 3. Fallback to standard discovery (GOOGLE_APPLICATION_CREDENTIALS or GCE metadata)
	return new google.auth.GoogleAuth({
		scopes: [
			"https://www.googleapis.com/auth/documents.readonly",
			"https://www.googleapis.com/auth/drive",
		],
	});
}

async function requireDocAdmin() {
	const session = await getServerSession();
	if (!session?.user) {
		return { error: "Non authentifié" as const, user: null };
	}

	const supabase = await createClient();
	const { data: profile } = await supabase
		.from("profiles")
		.select("role")
		.eq("id", session.user.id)
		.single();

	if (!profile || !isAdminRole(profile.role)) {
		return { error: "Permissions insuffisantes" as const, user: null };
	}

	return { error: null, user: session.user };
}

export async function importGoogleDoc(docId: string) {
	try {
		const { error: authError, user } = await requireDocAdmin();
		if (authError || !user) {
			return { success: false, error: authError || "Non authentifié" };
		}

		const auth = getGoogleAuth();
		const docs = google.docs({
			auth,
			version: "v1",
		});

		const docResponse = await docs.documents.get({ documentId: docId });
		const doc = docResponse.data;

		if (!doc.title || !doc.body) {
			return { error: "Document invalide ou vide", success: false };
		}

		const htmlContent = convertGoogleDocToHtml(doc);

		const baseSlug = (doc.title || "sans-titre")
			.toLowerCase()
			.replaceAll(/[^a-z0-9]+/g, "-")
			.replaceAll(/^-|-$/g, "");
		const slug = `${baseSlug}-${Date.now().toString().slice(-6)}`;

		// Use Admin Client to bypass RLS
		const supabase = createAdminClient();
		const { data, error } = await (supabase as any)
			.from("articles")
			.insert({
				author_id: user.id,
				category: "Import Google Doc",
				content: htmlContent,
				excerpt: "Importé depuis Google Docs",
				slug: slug,
				status: "draft",
				title: doc.title,
			})
			.select()
			.single();

		if (error) {
			console.error("Supabase Insert Error:", error);
			return { error: `Erreur base de données: ${error.message}`, success: false };
		}

		revalidatePath("/dashboard/news");
		return { data, success: true };
	} catch (error: unknown) {
		const err = error as { code?: number; message?: string };
		console.error("Google Import Error:", err);
		if (err.code === 403) {
			return { success: false, error: "Accès refusé. Document non public." };
		}
		return { error: err.message || "Erreur lors de l'import.", success: false };
	}
}

/**
 * Importer tous les Google Docs d'un dossier spécifique
 */
export async function importDriveFolder() {
	try {
		const { error: authError } = await requireDocAdmin();
		if (authError) {
			return { success: false, error: authError };
		}

		const auth = getGoogleAuth();
		const drive = google.drive({ auth, version: "v3" });

		// Lister les fichiers Google Docs dans le dossier
		const res = await drive.files.list({
			fields: "files(id, name)",
			q: `'${DRIVE_FOLDER_ID}' in parents and mimeType = 'application/vnd.google-apps.document' and trashed = false`,
		});

		const files = res.data.files || [];
		if (files.length === 0) {
			return { success: true, message: "Aucun document trouvé dans le dossier." };
		}

		let importedCount = 0;
		for (const file of files) {
			if (file.id) {
				const result = await importGoogleDoc(file.id);
				if (result.success) {
					importedCount++;
				}
			}
		}

		revalidatePath("/dashboard/news");
		return { message: `${importedCount} articles importés avec succès.`, success: true };
	} catch (error: unknown) {
		const err = error as { message?: string };
		console.error("Drive Import Error:", err);
		return { error: err.message || "Erreur lors de l'importation du dossier.", success: false };
	}
}

/**
 * Exporter un article vers Google Drive (en tant que nouveau Google Doc)
 */
export async function exportToGoogleDrive(newsId: string) {
	try {
		const { error: authError } = await requireDocAdmin();
		if (authError) {
			return { success: false, error: authError };
		}

		const supabase = await createClient();
		const { data: news, error: fetchError } = await supabase
			.from("articles")
			.select("*")
			.eq("id", newsId)
			.single();

		if (fetchError || !news) {
			return { success: false, error: "Article introuvable." };
		}

		const item = news as any;
		const auth = getGoogleAuth();
		const drive = google.drive({ auth, version: "v3" });

		const res = await drive.files.create({
			media: {
				body: item.content,
				mimeType: "text/html",
			},
			requestBody: {
				mimeType: "application/vnd.google-apps.document",
				name: `[Export] ${item.title}`,
				parents: [DRIVE_FOLDER_ID],
			},
		});

		return { fileId: res.data.id, success: true };
	} catch (error: unknown) {
		const err = error as { message?: string };
		console.error("Drive Export Error:", err);
		return { error: err.message || "Erreur lors de l'exportation.", success: false };
	}
}
