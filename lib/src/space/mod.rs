//! Space is a storage for atoms with search queries execution capability.
//! This module is intended to keep different space implementations.

pub mod grounding;
pub mod module;

use std::fmt::Display;
use std::rc::{Rc, Weak};
use std::cell::{RefCell, Ref, RefMut};
use std::borrow::Cow;

use crate::common::FlexRef;
use crate::atom::*;
use crate::atom::matcher::{BindingsSet, apply_bindings_to_atom_move};

/// Contains information about space modification event.
#[derive(Clone, Debug, PartialEq)]
pub enum SpaceEvent {
    /// Atom is added into a space.
    Add(Atom),
    /// Atom is removed from space.
    Remove(Atom),
    /// First atom is replaced by the second one.
    Replace(Atom, Atom),
}

/// Space modification event observer trait.
///
/// # Examples
///
/// ```
/// use hyperon::sym;
/// use hyperon::space::*;
/// use hyperon::space::grounding::*;
/// use std::rc::Rc;
/// use std::cell::RefCell;
///
/// struct MyObserver {
///     events: Vec<SpaceEvent>
/// }
///
/// impl SpaceObserver for MyObserver {
///     fn notify(&mut self, event: &SpaceEvent) {
///         self.events.push(event.clone());
///     }
/// }
///
/// let mut space = GroundingSpace::new();
/// let observer = space.common().register_observer(MyObserver{ events: Vec::new() });
/// 
/// space.add(sym!("A"));
/// space.replace(&sym!("A"), sym!("B"));
/// space.remove(&sym!("B"));
///
/// assert_eq!(observer.borrow().events, vec![SpaceEvent::Add(sym!("A")),
///     SpaceEvent::Replace(sym!("A"), sym!("B")),
///     SpaceEvent::Remove(sym!("B"))]);
/// ```
pub trait SpaceObserver {
    /// Notifies about space modification.
    fn notify(&mut self, event: &SpaceEvent);
}

/// A reference to a SpaceObserver that has been registered with a Space
#[derive(Clone)]
pub struct SpaceObserverRef<T: SpaceObserver> (Rc<RefCell<T>>);

impl<T: SpaceObserver> SpaceObserverRef<T> {
    /// Returns a [Ref] to mutably access the [SpaceObserver]
    pub fn borrow(&self) -> Ref<T> {
        self.0.borrow()
    }
    /// Returns a [RefMut] to mutably access the [SpaceObserver]
    pub fn borrow_mut(&self) -> RefMut<T> {
        self.0.borrow_mut()
    }
    /// Returns the contents of the `SpaceObserverRef`
    ///
    /// This method is used in the implementation of the C API bindings, and is probably
    /// not necessary for Rust API clients
    pub fn into_inner(self) -> Rc<RefCell<T>> {
        self.0
    }
}

impl<T: SpaceObserver> From<Rc<RefCell<T>>> for SpaceObserverRef<T> {
    fn from(observer: Rc<RefCell<T>>) -> Self {
        Self(observer)
    }
}

/// A common object that needs to be maintained by all objects implementing the Space trait
#[derive(Default)]
pub struct SpaceCommon {
    observers: RefCell<Vec<Weak<RefCell<dyn SpaceObserver>>>>,
}
impl SpaceCommon {
    /// Registers space modifications `observer`. Observer is automatically deregistered when
    /// the returned [SpaceObserverRef] and any clones are dropped.
    /// 
    /// See [SpaceObserver] for usage example.
    pub fn register_observer<T: SpaceObserver + 'static>(&self, observer: T) -> SpaceObserverRef<T> {
        let observer_ref = Rc::new(RefCell::new(observer));
        self.observers.borrow_mut().push(Rc::downgrade(&observer_ref) as Weak<RefCell<dyn SpaceObserver>>);
        SpaceObserverRef(observer_ref)
    }

    /// Notifies all registered observers about space modification `event`.
    pub fn notify_all_observers(&self, event: &SpaceEvent) {
        let mut cleanup = false;
        for observer in self.observers.borrow_mut().iter() {
            if let Some(observer) = observer.upgrade() {
                observer.borrow_mut().notify(event);
            } else {
                cleanup = true;
            }
        }
        if cleanup {
            self.observers.borrow_mut().retain(|w| w.strong_count() > 0);
        }
    }
}

impl Clone for SpaceCommon {
    fn clone(&self) -> Self {
        Self {
            //We don't want to clone observers when a space is cloned, as that leads to a situation
            // where an observer can't know which space an event pertains to
            observers: RefCell::new(vec![]),
        }
    }
}

/// An interface for visiting space atoms.
pub trait SpaceVisitor {
    /// Method is called by [Space::visit] implementation for each atom from the atomspace.
    fn accept(&mut self, atom: Cow<Atom>);
}

/// One can use closure instead of implementing [SpaceVisitor] interface manually.
impl<T> SpaceVisitor for T where T: FnMut(Cow<Atom>) {
    fn accept(&mut self, atom: Cow<Atom>) {
        (*self)(atom)
    }
}

/// Read-only space trait.
pub trait Space: std::fmt::Debug + std::fmt::Display {
    /// Access the SpaceCommon object owned by the Space
    fn common(&self) -> FlexRef<SpaceCommon>;

    /// Executes `query` on the space and returns variable bindings found.
    /// Query may include sub-queries glued by [grounding::COMMA_SYMBOL] symbol. 
    /// Each [Bindings](crate::atom::matcher::Bindings) instance in the returned [BindingsSet]
    /// represents single result.
    ///
    /// # Examples
    ///
    /// ```
    /// use hyperon::{expr, bind_set, sym};
    /// use hyperon::matcher::BindingsSet;
    /// use hyperon::space::grounding::GroundingSpace;
    ///
    /// let space = GroundingSpace::from_vec(vec![expr!("A" "B"), expr!("B" "C")]);
    /// let query = expr!("," ("A" x) (x "C"));
    ///
    /// let result = space.query(&query);
    ///
    /// assert_eq!(result, bind_set![{x: sym!("B")}]);
    /// ```
    fn query(&self, query: &Atom) -> BindingsSet;

    /// Executes `pattern` query on the space and for each result substitutes
    /// variables in `template` by the values from `pattern`. Returns results
    /// of the substitution.
    ///
    /// # Examples
    ///
    /// ```
    /// use hyperon::{expr, assert_eq_no_order};
    /// use hyperon::space::Space;
    /// use hyperon::space::grounding::GroundingSpace;
    ///
    /// let space = GroundingSpace::from_vec(vec![expr!("A" "B"), expr!("A" "C")]);
    ///
    /// let result = space.subst(&expr!("A" x), &expr!("D" x));
    ///
    /// assert_eq_no_order!(result, vec![expr!("D" "B"), expr!("D" "C")]);
    /// ```
    fn subst(&self, pattern: &Atom, template: &Atom) -> Vec<Atom> {
        self.query(pattern).drain(0..)
            .map(| bindings | apply_bindings_to_atom_move(template.clone(), &bindings))
            .collect()
    }

    /// Returns the number of Atoms in the space, or None if this can't be determined
    fn atom_count(&self) -> Option<usize> {
        None
    }

    /// Returns an iterator over every atom in the space, or None if that is not possible
    fn atom_iter(&self) -> Option<Box<dyn Iterator<Item=&Atom> + '_>> {
        None
    }

    /// Visit each atom of the space and call [SpaceVisitor::accept] method.
    /// This method is optional. Return `Err(())` if method is not implemented.
    /// `Cow<Atom>` is used to allow passing both references and values. First
    /// is appropriate for collection based atomspace. Second one is more
    /// usable if atomspace is a generator or when values cannot be extracted
    /// easily and should be reconstructed instead.
    fn visit(&self, v: &mut dyn SpaceVisitor) -> Result<(), ()>;

    /// Returns an `&dyn `[Any](std::any::Any) for spaces where this is possible
    fn as_any(&self) -> Option<&dyn std::any::Any>;

    /// Returns an `&mut dyn `[Any](std::any::Any) for spaces where this is possible
    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any>;
}

/// Mutable space trait.
pub trait SpaceMut: Space {
    /// Adds `atom` into space.
    ///
    /// # Examples
    ///
    /// ```
    /// use hyperon::sym;
    /// use hyperon::space::grounding::GroundingSpace;
    /// use hyperon::atom::matcher::BindingsSet;
    ///
    /// let mut space = GroundingSpace::from_vec(vec![sym!("A")]);
    /// 
    /// space.add(sym!("B"));
    ///
    /// assert_eq!(space.query(&sym!("A")), BindingsSet::single());
    /// assert_eq!(space.query(&sym!("B")), BindingsSet::single());
    /// assert_eq!(space.query(&sym!("C")), BindingsSet::empty());
    /// ```
    fn add(&mut self, atom: Atom);

    /// Removes `atom` from space. Returns true if atom was found and removed,
    /// and false otherwise.
    ///
    /// # Examples
    ///
    /// ```
    /// use hyperon::sym;
    /// use hyperon::matcher::BindingsSet;
    /// use hyperon::space::grounding::GroundingSpace;
    ///
    /// let mut space = GroundingSpace::from_vec(vec![sym!("A")]);
    /// 
    /// space.remove(&sym!("A"));
    ///
    /// assert_eq!(space.query(&sym!("A")), BindingsSet::empty());
    /// ```
    fn remove(&mut self, atom: &Atom) -> bool;

    /// Replaces `from` atom to `to` atom inside space. Doesn't add `to` when
    /// `from` is not found. Returns true if atom was found and replaced, and
    /// false otherwise.
    ///
    /// # Examples
    ///
    /// ```
    /// use hyperon::sym;
    /// use hyperon::space::grounding::GroundingSpace;
    /// use hyperon::atom::matcher::BindingsSet;
    ///
    /// let mut space = GroundingSpace::from_vec(vec![sym!("A")]);
    /// 
    /// space.replace(&sym!("A"), sym!("B"));
    ///
    /// assert_eq!(space.query(&sym!("A")), BindingsSet::empty());
    /// assert_eq!(space.query(&sym!("B")), BindingsSet::single());
    /// ```
    fn replace(&mut self, from: &Atom, to: Atom) -> bool;

    /// Turn a &dyn SpaceMut into an &dyn Space.  Obsolete when Trait Upcasting is stabilized.
    /// [Rust issue #65991](https://github.com/rust-lang/rust/issues/65991)  Any month now.
    fn as_space<'a>(&self) -> &(dyn Space + 'a);
}

#[derive(Clone)]
pub struct DynSpace(Rc<RefCell<dyn SpaceMut>>);

impl DynSpace {
    pub fn with_rc(space: Rc<RefCell<dyn SpaceMut>>) -> Self {
        Self(space)
    }
    pub fn new<T: SpaceMut + 'static>(space: T) -> Self {
        let shared = Rc::new(RefCell::new(space));
        DynSpace(shared)
    }
    pub fn borrow(&self) -> Ref<dyn SpaceMut> {
        self.0.borrow()
    }
    pub fn borrow_mut(&self) -> RefMut<dyn SpaceMut> {
        self.0.borrow_mut()
    }
    /// A convenience.  See [SpaceCommon::register_observer]
    pub fn register_observer<T: SpaceObserver + 'static>(&self, observer: T) -> SpaceObserverRef<T> {
        self.common().register_observer(observer)
    }
}

impl core::fmt::Debug for DynSpace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

impl Display for DynSpace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", &*self.0.borrow())
    }
}

impl SpaceMut for DynSpace {
    fn add(&mut self, atom: Atom) {
        self.0.borrow_mut().add(atom)
    }
    fn remove(&mut self, atom: &Atom) -> bool {
        self.0.borrow_mut().remove(atom)
    }
    fn replace(&mut self, from: &Atom, to: Atom) -> bool {
        self.0.borrow_mut().replace(from, to)
    }
    fn as_space<'a>(&self) -> &(dyn Space + 'a) {
        self
    }
}

impl Space for DynSpace {
    fn common(&self) -> FlexRef<SpaceCommon> {
        FlexRef::from_ref_cell(Ref::map(self.0.borrow(), |space| space.common().into_simple()))
    }
    fn query(&self, query: &Atom) -> BindingsSet {
        self.0.borrow().query(query)
    }
    fn subst(&self, pattern: &Atom, template: &Atom) -> Vec<Atom> {
        self.0.borrow().subst(pattern, template)
    }
    fn atom_count(&self) -> Option<usize> {
        self.0.borrow().atom_count()
    }
    fn atom_iter(&self) -> Option<Box<dyn Iterator<Item=&Atom> + '_>> {
        None
    }
    fn visit(&self, v: &mut dyn SpaceVisitor) -> Result<(), ()> {
        self.0.borrow().visit(v)
    }
    fn as_any(&self) -> Option<&dyn std::any::Any> {
        None
    }
    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        None
    }
}

impl PartialEq for DynSpace {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::addr_eq(RefCell::as_ptr(&self.0), RefCell::as_ptr(&other.0))
    }
}

impl crate::atom::Grounded for DynSpace {
    fn type_(&self) -> Atom {
        rust_type_atom::<DynSpace>()
    }

    fn as_match(&self) -> Option<&dyn CustomMatch> {
        Some(self)
    }
}

impl CustomMatch for DynSpace {
    fn match_(&self, other: &Atom) -> matcher::MatchResultIter {
        Box::new(self.query(other).into_iter())
    }
}

impl<T: Space> Space for &T {
    fn common(&self) -> FlexRef<SpaceCommon> {
        T::common(*self)
    }
    fn query(&self, query: &Atom) -> BindingsSet {
        T::query(*self, query)
    }
    fn subst(&self, pattern: &Atom, template: &Atom) -> Vec<Atom> {
        T::subst(*self, pattern, template)
    }
    fn atom_count(&self) -> Option<usize> {
        T::atom_count(*self)
    }
    fn atom_iter(&self) -> Option<Box<dyn Iterator<Item=&Atom> + '_>> {
        T::atom_iter(*self)
    }
    fn visit(&self, v: &mut dyn SpaceVisitor) -> Result<(), ()> {
        T::visit(*self, v)
    }
    fn as_any(&self) -> Option<&dyn std::any::Any> {
        None
    }
    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        None
    }
}

