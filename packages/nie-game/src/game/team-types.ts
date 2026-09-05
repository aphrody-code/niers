export interface TeamMemberStats {
	kick: number;
	control: number;
	technique: number;
	pressure: number;
	physical: number;
	agility: number;
	intelligence: number;
}

export interface TeamMember {
	/** Slot key: "field-0" to "field-10", "reserve-0" to "reserve-4", "manager-0", "support-0" to "support-2" */
	slot: string;
	charaId: string;
	name: string;
	position: string;
	element: string;
	rarity: string;
	imageUrl: string;
	slug: string;
	stats?: TeamMemberStats;
	internalCode?: string;
	zukanHash?: string;
}

export interface TeamData {
	formationId: string;
	members: TeamMember[];
}

export interface SavedTeam {
	id: string;
	name: string;
	formation_id: string;
	formation_data: TeamData;
	is_public: boolean;
	created_at: string;
	updated_at: string;
}
