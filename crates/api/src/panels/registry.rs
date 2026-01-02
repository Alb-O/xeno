//! Panel instance registry using trait object storage.

use std::collections::HashMap;

use evildoer_registry::panels::{
	find_factory, find_panel, panel_kind_index, PanelId, SplitBuffer,
};

/// Runtime registry for panel instances.
///
/// Manages the lifecycle of panel instances, including creation, storage,
/// and access. Each panel type can have multiple instances (unless marked
/// as singleton in its definition).
///
/// Panels are stored as trait objects (`Box<dyn SplitBuffer>`) providing
/// uniform access to all panel operations without type dispatch.
pub struct PanelRegistry {
	instances: HashMap<PanelId, Box<dyn SplitBuffer>>,
	next_instance: HashMap<u16, u16>,
}

impl Default for PanelRegistry {
	fn default() -> Self {
		Self::new()
	}
}

impl PanelRegistry {
	/// Creates an empty panel registry.
	pub fn new() -> Self {
		Self {
			instances: HashMap::new(),
			next_instance: HashMap::new(),
		}
	}

	/// Creates or returns the existing singleton panel of the given type.
	///
	/// For singleton panels, returns the existing instance if one exists.
	/// For non-singleton panels, always creates a new instance.
	///
	/// Returns `None` if the panel type is not registered or has no factory.
	pub fn get_or_create(&mut self, name: &str) -> Option<PanelId> {
		let def = find_panel(name)?;
		let kind = panel_kind_index(name)?;

		if def.singleton
			&& let Some(id) = self.find_by_kind(kind)
		{
			return Some(id);
		}

		let factory = find_factory(name)?;
		let instance = self.next_instance.entry(kind).or_insert(0);
		let id = PanelId::new(kind, *instance);
		*instance += 1;

		let mut panel = (factory.factory)();
		panel.on_open();
		self.instances.insert(id, panel);
		Some(id)
	}

	/// Inserts a panel instance directly.
	///
	/// Useful when the panel is created externally (e.g., by the Editor).
	pub fn insert<T: SplitBuffer + 'static>(&mut self, name: &str, panel: T) -> Option<PanelId> {
		let kind = panel_kind_index(name)?;
		let instance = self.next_instance.entry(kind).or_insert(0);
		let id = PanelId::new(kind, *instance);
		*instance += 1;

		self.instances.insert(id, Box::new(panel));
		Some(id)
	}

	/// Finds an existing panel instance by kind.
	pub fn find_by_kind(&self, kind: u16) -> Option<PanelId> {
		self.instances.keys().find(|id| id.kind == kind).copied()
	}

	/// Finds an existing panel instance by name.
	pub fn find_by_name(&self, name: &str) -> Option<PanelId> {
		self.find_by_kind(panel_kind_index(name)?)
	}

	/// Returns all panel IDs of a given type.
	pub fn all_of_kind(&self, kind: u16) -> impl Iterator<Item = PanelId> + '_ {
		self.instances
			.keys()
			.filter(move |id| id.kind == kind)
			.copied()
	}

	/// Returns a reference to a panel by ID.
	pub fn get(&self, id: PanelId) -> Option<&(dyn SplitBuffer + 'static)> {
		self.instances.get(&id).map(|b| &**b)
	}

	/// Returns a mutable reference to a panel by ID.
	pub fn get_mut(&mut self, id: PanelId) -> Option<&mut (dyn SplitBuffer + 'static)> {
		self.instances.get_mut(&id).map(|b| &mut **b)
	}

	/// Removes a panel by ID.
	pub fn remove(&mut self, id: PanelId) -> Option<Box<dyn SplitBuffer>> {
		self.instances.remove(&id)
	}

	/// Returns true if a panel with the given ID exists.
	pub fn contains(&self, id: PanelId) -> bool {
		self.instances.contains_key(&id)
	}

	/// Returns the number of panel instances.
	pub fn len(&self) -> usize {
		self.instances.len()
	}

	/// Returns true if there are no panel instances.
	pub fn is_empty(&self) -> bool {
		self.instances.is_empty()
	}

	/// Returns all panel IDs.
	pub fn ids(&self) -> impl Iterator<Item = PanelId> + '_ {
		self.instances.keys().copied()
	}
}
