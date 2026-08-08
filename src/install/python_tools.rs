use std::{fs, path::Path, process::Command};

use super::{
    InstallError, Installer,
    common::{PipOptions, ScratchDir},
};
use crate::tool_definitions::Tool;

struct FetchedScript {
    name: &'static str,
    url: &'static str,
}

struct UvRecipe {
    slug: &'static str,
    python: &'static str,
    requirements: &'static [&'static str],
    scripts: &'static [&'static str],
    torch: &'static [&'static str],
    extra_indexes: &'static [&'static str],
    index_strategy: Option<&'static str>,
    no_build_isolation: bool,
    extra_env: &'static [(&'static str, &'static str)],
    fetched_scripts: &'static [FetchedScript],
    gpu_probe: Option<&'static str>,
    verify: Option<(&'static str, &'static [&'static str])>,
}

impl UvRecipe {
    const fn simple(
        slug: &'static str,
        python: &'static str,
        requirements: &'static [&'static str],
        scripts: &'static [&'static str],
    ) -> Self {
        Self {
            slug,
            python,
            requirements,
            scripts,
            torch: &[],
            extra_indexes: &[],
            index_strategy: None,
            no_build_isolation: false,
            extra_env: &[],
            fetched_scripts: &[],
            gpu_probe: None,
            verify: None,
        }
    }
}

pub(super) fn install(installer: &mut Installer, tool: Tool) -> Result<(), InstallError> {
    match tool {
        Tool::Chai1 => install_recipe(
            installer,
            UvRecipe {
                extra_indexes: &["https://download.pytorch.org/whl/cu124"],
                gpu_probe: Some(
                    "import torch; assert torch.cuda.is_available() and \
                     torch.cuda.is_bf16_supported(), 'Chai-1 requires a CUDA GPU with bfloat16'",
                ),
                ..UvRecipe::simple(
                    Tool::Chai1.slug(),
                    "3.11",
                    &["chai_lab==0.6.1"],
                    &["chai-lab"],
                )
            },
        ),
        Tool::Protenix => install_recipe(
            installer,
            UvRecipe {
                gpu_probe: Some(
                    "import torch; assert torch.cuda.is_available(), 'Protenix requires CUDA'",
                ),
                ..UvRecipe::simple(Tool::Protenix.slug(), "3.11", &["protenix"], &["protenix"])
            },
        ),
        Tool::EsmFold2 => install_esmfold(installer),
        Tool::ImmuneBuilder => install_recipe(
            installer,
            UvRecipe::simple(
                Tool::ImmuneBuilder.slug(),
                "3.11",
                &["ImmuneBuilder", "openmm", "pdbfixer", "anarci"],
                &["ABodyBuilder2", "NanoBodyBuilder2", "TCRBuilder2"],
            ),
        ),
        Tool::BioPhi => install_recipe(
            installer,
            UvRecipe::simple(
                Tool::BioPhi.slug(),
                "3.11",
                &[
                    "biophi @ git+https://github.com/Merck/BioPhi@main",
                    "abnumber",
                ],
                &["biophi"],
            ),
        ),
        Tool::ProteinMpnnDdg => install_proteinmpnn_ddg(installer),
        Tool::RfDiffusion => install_rfdiffusion(installer),
        Tool::RfAntibody => install_rfantibody(installer),
        Tool::IgDesign => install_igdesign(installer),
        Tool::ThermoMpnn => install_checkout_recipe(
            installer,
            UvRecipe {
                torch: &["torch==2.7.1"],
                ..UvRecipe::simple(
                    Tool::ThermoMpnn.slug(),
                    "3.12",
                    &[
                        "numpy<2",
                        "pandas",
                        "biopython",
                        "tqdm",
                        "omegaconf",
                        "pytorch-lightning",
                    ],
                    &[],
                )
            },
            "https://github.com/Kuhlman-Lab/ThermoMPNN",
            "ThermoMPNN",
        ),
        Tool::DeepSp => install_deepsp(installer),
        Tool::DeepImmuno => install_checkout_recipe(
            installer,
            UvRecipe::simple(
                Tool::DeepImmuno.slug(),
                "3.10",
                &["tensorflow<2.16", "pandas", "numpy<2", "scikit-learn"],
                &[],
            ),
            "https://github.com/frankligy/DeepImmuno",
            "DeepImmuno",
        ),
        Tool::TlImmuno2 => install_checkout_recipe(
            installer,
            UvRecipe::simple(
                Tool::TlImmuno2.slug(),
                "3.10",
                &[
                    "tensorflow<2.16",
                    "pandas",
                    "pyarrow",
                    "numpy<2",
                    "scikit-learn",
                ],
                &[],
            ),
            "https://github.com/XSLiuLab/TLimmuno2",
            "TLimmuno2",
        ),
        Tool::NetSolP => install_netsolp(installer),
        Tool::DeepStabP => install_deepstabp(installer),
        Tool::DlkCat => install_checkout_recipe(
            installer,
            UvRecipe {
                torch: &["torch==2.7.1"],
                ..UvRecipe::simple(
                    Tool::DlkCat.slug(),
                    "3.10",
                    &["numpy<2", "rdkit", "scikit-learn"],
                    &[],
                )
            },
            "https://github.com/SysBioChalmers/DLKcat",
            "DLKcat",
        ),
        Tool::CatPred => install_catpred(installer),
        Tool::Anarcii => install_recipe(
            installer,
            UvRecipe {
                torch: &["torch==2.7.1"],
                verify: Some((
                    "python",
                    &[
                        "-c",
                        "import anarcii; print('anarcii', anarcii.__version__)",
                    ],
                )),
                ..UvRecipe::simple(Tool::Anarcii.slug(), "3.12", &["anarcii"], &["anarcii"])
            },
        ),
        Tool::Placer => install_placer(installer),
        Tool::Gromacs => install_gromacs(installer),
        Tool::BoltzAdme => install_recipe(
            installer,
            UvRecipe::simple(Tool::BoltzAdme.slug(), "3.13", &["boltz-api~=0.45.0"], &[]),
        ),
        _ => Err(InstallError::InvalidConfiguration(format!(
            "{} has no uv/executable recipe",
            tool.name()
        ))),
    }
}

fn install_recipe(installer: &mut Installer, recipe: UvRecipe) -> Result<(), InstallError> {
    let backend = (!recipe.torch.is_empty())
        .then(|| installer.select_torch_backend())
        .transpose()?;
    installer.create_venv(recipe.slug, recipe.python)?;
    if let Some(backend) = backend {
        installer.install_torch(recipe.slug, recipe.torch, backend)?;
    }
    installer.pip_install(
        recipe.slug,
        recipe.requirements,
        PipOptions {
            extra_indexes: recipe.extra_indexes,
            index_strategy: recipe.index_strategy,
            no_build_isolation: recipe.no_build_isolation,
            extra_env: recipe.extra_env,
            ..PipOptions::default()
        },
    )?;
    for script in recipe.fetched_scripts {
        installer.install_fetched_python_script(recipe.slug, script.url, script.name)?;
    }
    for script in recipe.scripts {
        let path = installer.venv_script(recipe.slug, script);
        if !path.is_file() {
            return Err(InstallError::InvalidConfiguration(format!(
                "{} installed, but {script} was not created in its environment",
                recipe.slug
            )));
        }
    }
    if let Some(probe) = recipe.gpu_probe {
        let mut command = Command::new(installer.venv_python(recipe.slug));
        command.args(["-c", probe]);
        installer.checked(&mut command)?;
    }
    if let Some((script, arguments)) = recipe.verify {
        let executable = if script == "python" {
            installer.venv_python(recipe.slug)
        } else {
            installer.venv_script(recipe.slug, script)
        };
        let mut command = Command::new(executable);
        command.args(arguments);
        installer.checked(&mut command)?;
    }
    Ok(())
}

fn install_checkout_recipe(
    installer: &mut Installer,
    recipe: UvRecipe,
    url: &str,
    directory: &str,
) -> Result<(), InstallError> {
    install_recipe(installer, recipe)?;
    let target = installer.tools_root().join(directory);
    installer.clone_or_update(url, &target)
}

fn install_esmfold(installer: &mut Installer) -> Result<(), InstallError> {
    install_recipe(
        installer,
        UvRecipe {
            torch: &["torch==2.7.1"],
            no_build_isolation: true,
            extra_env: &[("NVCC_APPEND_FLAGS", "-std=c++17")],
            fetched_scripts: &[FetchedScript {
                name: "esm-fold",
                url: "https://raw.githubusercontent.com/facebookresearch/esm/v2.0.0/scripts/esmfold_inference.py",
            }],
            ..UvRecipe::simple(
                Tool::EsmFold2.slug(),
                "3.11",
                &[
                    "fair-esm[esmfold]~=2.0.0",
                    "openfold @ git+https://github.com/aqlaboratory/openfold.git@4b41059694619831a7db195b7e0988fc4ff3a307",
                ],
                &["esm-fold"],
            )
        },
    )
}

fn install_proteinmpnn_ddg(installer: &mut Installer) -> Result<(), InstallError> {
    install_recipe(
        installer,
        UvRecipe {
            fetched_scripts: &[FetchedScript {
                name: "proteinmpnn-ddg",
                url: "https://raw.githubusercontent.com/PeptoneLtd/proteinmpnn_ddg/main/predict.py",
            }],
            gpu_probe: Some(
                "import jax; assert any(d.platform == 'gpu' for d in jax.devices()), \
                 'ProteinMPNN-ddG requires a JAX CUDA device'",
            ),
            ..UvRecipe::simple(
                Tool::ProteinMpnnDdg.slug(),
                "3.10",
                &[
                    "ProteinMPNN-ddG[cuda12] @ git+https://github.com/PeptoneLtd/proteinmpnn_ddg.git@main",
                    "dm-haiku==0.0.13",
                ],
                &["proteinmpnn-ddg"],
            )
        },
    )
}

fn install_rfdiffusion(installer: &mut Installer) -> Result<(), InstallError> {
    install_recipe(
        installer,
        UvRecipe {
            extra_indexes: &["https://download.pytorch.org/whl/cu118"],
            index_strategy: Some("unsafe-best-match"),
            gpu_probe: Some(
                "import torch; assert torch.cuda.is_available(), 'RFdiffusion requires CUDA'",
            ),
            ..UvRecipe::simple(
                Tool::RfDiffusion.slug(),
                "3.10",
                &[
                    "dgl @ https://data.dgl.ai/wheels/torch-2.3/cu118/dgl-2.4.0%2Bcu118-cp310-cp310-manylinux1_x86_64.whl",
                    "numpy<2",
                    "e3nn==0.3.3",
                    "hydra-core",
                    "icecream",
                    "opt_einsum",
                    "scipy",
                    "pandas",
                    "decorator",
                    "pyrsistent",
                    "dllogger @ git+https://github.com/NVIDIA/dllogger.git@master",
                    "se3-transformer @ git+https://github.com/RosettaCommons/RFdiffusion.git@main#subdirectory=env/SE3Transformer",
                    "rfdiffusion @ git+https://github.com/RosettaCommons/RFdiffusion.git@main",
                ],
                &[],
            )
        },
    )?;
    let target = installer.tools_root().join("RFdiffusion");
    installer.clone_or_update("https://github.com/RosettaCommons/RFdiffusion", &target)?;
    let weights = target.join("models");
    for (directory, filename) in [
        ("6f5902ac237024bdd0c176cb93063dc4", "Base_ckpt.pt"),
        ("e29311f6f1bf1af907f9ef9f44b8328b", "Complex_base_ckpt.pt"),
        (
            "60f09a193fb5e5ccdc4980417708dbab",
            "Complex_Fold_base_ckpt.pt",
        ),
        ("74f51cfb8b440f50d70878e05361d8f0", "InpaintSeq_ckpt.pt"),
        (
            "76d00716416567174cdb7ca96e208296",
            "InpaintSeq_Fold_ckpt.pt",
        ),
        ("5532d2e1f3a4738decd58b19d633b3c3", "ActiveSite_ckpt.pt"),
        ("12fc204edeae5b57713c5ad7dcb97d39", "Base_epoch8_ckpt.pt"),
    ] {
        installer.download(
            &format!("https://files.ipd.uw.edu/pub/RFdiffusion/{directory}/{filename}"),
            &weights.join(filename),
        )?;
    }
    Ok(())
}

fn install_rfantibody(installer: &mut Installer) -> Result<(), InstallError> {
    install_recipe(
        installer,
        UvRecipe {
            extra_indexes: &["https://download.pytorch.org/whl/cu118"],
            index_strategy: Some("unsafe-best-match"),
            gpu_probe: Some(
                "import torch; assert torch.cuda.is_available(), 'RFantibody requires CUDA'",
            ),
            ..UvRecipe::simple(
                Tool::RfAntibody.slug(),
                "3.10",
                &[
                    "dgl @ https://data.dgl.ai/wheels/torch-2.3/cu118/dgl-2.4.0%2Bcu118-cp310-cp310-manylinux1_x86_64.whl",
                    "rfantibody @ git+https://github.com/RosettaCommons/RFantibody.git@main",
                ],
                &["rfdiffusion", "proteinmpnn", "rf2"],
            )
        },
    )?;
    let target = installer.tools_root().join("RFantibody");
    installer.clone_or_update("https://github.com/RosettaCommons/RFantibody", &target)?;
    let weights = target.join("weights");
    for (url, filename) in [
        (
            "https://files.ipd.uw.edu/pub/RFantibody/RFdiffusion_Ab.pt",
            "RFdiffusion_Ab.pt",
        ),
        (
            "https://files.ipd.uw.edu/pub/RFantibody/ProteinMPNN_v48_noise_0.2.pt",
            "ProteinMPNN_v48_noise_0.2.pt",
        ),
        (
            "https://files.ipd.uw.edu/pub/RFantibody/RF2_ab.pt",
            "RF2_ab.pt",
        ),
        (
            "https://zenodo.org/records/17488258/files/RFab_noframework-nosidechains-5-10-23_trainingparamsadded.pt?download=1",
            "RFab_noframework-nosidechains-5-10-23_trainingparamsadded.pt",
        ),
    ] {
        installer.download(url, &weights.join(filename))?;
    }
    Ok(())
}

fn install_igdesign(installer: &mut Installer) -> Result<(), InstallError> {
    install_recipe(
        installer,
        UvRecipe {
            extra_indexes: &["https://download.pytorch.org/whl/cu118"],
            index_strategy: Some("unsafe-best-match"),
            gpu_probe: Some(
                "import torch; assert torch.cuda.is_available(), 'IgDesign requires CUDA'",
            ),
            ..UvRecipe::simple(
                Tool::IgDesign.slug(),
                "3.11",
                &[
                    "torch==2.7.1+cu118",
                    "pandas>=2.0,<2.1",
                    "numpy<2",
                    "setuptools<81",
                    "lightning>=2.0,<2.1",
                    "cytoolz>=0.12.3",
                    "einops>=0.8.0",
                    "hydra-core>=1.3.2",
                    "biopython>=1.84",
                    "datasets>=2.20.0",
                    "anarci",
                    "huggingface_hub>=0.24.5",
                    "transformers>=4.42.4",
                    "torchtyping>=0.1.4",
                ],
                &[],
            )
        },
    )?;
    let target = installer.tools_root().join("igdesign");

    installer.clone_or_update("https://github.com/AbSciBio/igdesign", &target)?;
    // Normal VCS installation exposes no modules; upstream documents an editable checkout.
    let target_argument = target.to_string_lossy().into_owned();
    installer.pip_install(
        Tool::IgDesign.slug(),
        &["-e", &target_argument],
        PipOptions::default(),
    )?;
    let download = target.join("download_ckpts.sh");
    if download.is_file() {
        installer.run_upstream_script(&download, &[], &target)?;
    } else {
        installer.note(format!(
            "IgDesign has no checkpoint downloader; place its checkpoints under {}",
            target.join("ckpts").display()
        ));
    }
    Ok(())
}

fn install_deepsp(installer: &mut Installer) -> Result<(), InstallError> {
    install_checkout_recipe(
        installer,
        UvRecipe {
            torch: &["torch==2.7.1"],
            ..UvRecipe::simple(
                Tool::DeepSp.slug(),
                "3.11",
                &["tensorflow", "pandas", "numpy", "biopython", "anarcii"],
                &[],
            )
        },
        "https://github.com/Lailabcode/DeepSP",
        "DeepSP",
    )?;
    copy_support_script(
        installer,
        "tool_scripts/deepsp_cli.py",
        "DeepSP/deepsp_cli.py",
    )
}

fn install_deepstabp(installer: &mut Installer) -> Result<(), InstallError> {
    install_checkout_recipe(
        installer,
        UvRecipe {
            torch: &["torch==2.7.1"],
            ..UvRecipe::simple(
                Tool::DeepStabP.slug(),
                "3.11",
                &[
                    "transformers<5",
                    "sentencepiece",
                    "protobuf",
                    "biopython",
                    "pandas",
                    "pytorch-lightning",
                ],
                &[],
            )
        },
        "https://github.com/CSBiology/deepStabP",
        "deepStabP",
    )?;
    copy_support_script(
        installer,
        "tool_scripts/deepstabp_cli.py",
        "deepStabP/src/Api/deepstabp_cli.py",
    )
}

fn install_netsolp(installer: &mut Installer) -> Result<(), InstallError> {
    install_checkout_recipe(
        installer,
        UvRecipe {
            torch: &["torch==2.7.1"],
            ..UvRecipe::simple(
                Tool::NetSolP.slug(),
                "3.11",
                &["fair-esm~=2.0.0", "pandas", "numpy<2"],
                &[],
            )
        },
        "https://github.com/tvinet/NetSolP-1.0",
        "NetSolP-1.0",
    )?;
    let models = installer
        .tools_root()
        .join("NetSolP-1.0/PredictionServer/models");
    if let Some(url) = installer.config.netsolp_models_url.clone() {
        let scratch = ScratchDir::new_in(installer.tools_root(), "netsolp-models")?;
        let archive = scratch.path().join("netsolp_models.tar.gz");
        installer.download(&url, &archive)?;
        installer.extract_archive(&archive, &models)?;
    } else {
        installer.note(
            "NetSolP model checkpoints require DTU licence acceptance; set NETSOLP_MODELS_URL \
             when an archive is available",
        );
    }
    Ok(())
}

fn copy_support_script(
    installer: &Installer,
    source: &str,
    destination: &str,
) -> Result<(), InstallError> {
    let Some(source) = installer.support_file(source) else {
        installer.note(format!(
            "Optional adapter helper {source} was not supplied; the upstream checkout is installed"
        ));
        return Ok(());
    };
    let destination = installer.tools_root().join(destination);
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            InstallError::io(format!("unable to create {}", parent.display()), error)
        })?;
    }
    fs::copy(&source, &destination).map_err(|error| {
        InstallError::io(
            format!(
                "unable to copy {} to {}",
                source.display(),
                destination.display()
            ),
            error,
        )
    })?;
    Ok(())
}

fn install_catpred(installer: &mut Installer) -> Result<(), InstallError> {
    install_recipe(
        installer,
        UvRecipe {
            extra_indexes: &["https://download.pytorch.org/whl/cu124"],
            ..UvRecipe::simple(
                Tool::CatPred.slug(),
                "3.10",
                &[
                    "catpred @ git+https://github.com/maranasgroup/CatPred.git@main",
                    "ipdb",
                    "fair-esm",
                    "progres",
                    "transformers",
                    "sentencepiece",
                    "seaborn",
                    "rotary_embedding_torch==0.6.5",
                    "faiss-cpu",
                    "torch-geometric",
                ],
                &[],
            )
        },
    )?;
    let target = installer.tools_root().join("CatPred");
    installer.clone_or_update("https://github.com/maranasgroup/CatPred", &target)?;
    let data = target.join("capsule_data");
    if !data.join("data/pretrained").is_dir() {
        installer.step("Downloading the CatPred checkpoint archive (about 10 GiB)");
        let scratch = ScratchDir::new_in(installer.tools_root(), "catpred")?;
        let archive = scratch.path().join("capsule_data_update.tar.gz");
        if let Err(first_error) = installer.download(
            "https://catpred.s3.us-east-1.amazonaws.com/capsule_data_update.tar.gz",
            &archive,
        ) {
            installer.note(format!(
                "The regional CatPred URL failed ({first_error}); trying the fallback"
            ));
            installer.download(
                "https://catpred.s3.amazonaws.com/capsule_data_update.tar.gz",
                &archive,
            )?;
        }
        installer.extract_archive(&archive, &data)?;
    }
    if data.join("data/pretrained").is_dir() {
        create_catpred_links(&target)?;
    } else {
        installer.note(
            "The CatPred checkpoint layout was not recognized; set a checkpoint directory manually",
        );
    }
    Ok(())
}

#[cfg(unix)]
fn create_catpred_links(target: &Path) -> Result<(), InstallError> {
    use std::os::unix::fs::symlink;

    let reproduce = target.join("capsule_data/data/pretrained/reproduce_checkpoints");
    let links = target.join("checkpoint_links");
    fs::create_dir_all(&links)
        .map_err(|error| InstallError::io("unable to create CatPred checkpoint links", error))?;
    for (name, source) in [
        ("kcat", reproduce.join("kcat/seed0/fold_0")),
        (
            "km",
            reproduce.join("km/seed0/seqemb36_attn6_esm_ens10/fold_0"),
        ),
        (
            "ki",
            reproduce.join("ki/seed0/seqemb36_attn6_ens10_Pretrained_egnnFeats/fold_0"),
        ),
    ] {
        let destination = links.join(name);
        if destination
            .symlink_metadata()
            .is_ok_and(|metadata| metadata.file_type().is_symlink() || metadata.is_file())
        {
            fs::remove_file(&destination).map_err(|error| {
                InstallError::io(
                    format!("unable to replace {}", destination.display()),
                    error,
                )
            })?;
        } else if destination.is_dir() {
            return Err(InstallError::InvalidConfiguration(format!(
                "{} is a real directory; refusing to replace it with a symlink",
                destination.display()
            )));
        }
        symlink(source, destination)
            .map_err(|error| InstallError::io("unable to link CatPred checkpoints", error))?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn create_catpred_links(_target: &Path) -> Result<(), InstallError> {
    Ok(())
}

fn install_placer(installer: &mut Installer) -> Result<(), InstallError> {
    install_recipe(
        installer,
        UvRecipe {
            extra_indexes: &["https://download.pytorch.org/whl/cu118"],
            index_strategy: Some("unsafe-best-match"),
            gpu_probe: Some(
                "import torch; assert torch.cuda.is_available(), 'PLACER requires CUDA'",
            ),
            ..UvRecipe::simple(
                Tool::Placer.slug(),
                "3.10",
                &[
                    "dgl @ https://data.dgl.ai/wheels/torch-2.3/cu118/dgl-2.4.0%2Bcu118-cp310-cp310-manylinux1_x86_64.whl",
                    "torch==2.3.1",
                    "opt_einsum==3.4.0",
                    "openbabel-wheel==3.1.1.22",
                    "networkx>=3.2",
                    "numpy<2",
                    "pandas==2.2.3",
                    // Upstream pins 0.5.4, but that release exists on conda-forge rather than
                    // PyPI. Installing its matching Git tag keeps this uv recipe resolvable.
                    "e3nn @ git+https://github.com/e3nn/e3nn.git@0.5.4",
                    // PLACER imports NVIDIA's DGL SE(3) Transformer. Upstream's Conda manifest
                    // installs it from this subdirectory; omitting it leaves a checkout that
                    // passes dependency resolution but fails as soon as the runner imports.
                    "se3-transformer @ git+https://github.com/NVIDIA/DeepLearningExamples.git@729963dd47e7c8bd462ad10bfac7a7b0b604e6dd#subdirectory=DGLPyTorch/DrugDiscovery/SE3Transformer",
                ],
                &[],
            )
        },
    )?;
    let target = installer.tools_root().join("PLACER");
    installer.clone_or_update("https://github.com/baker-laboratory/PLACER", &target)
}

fn install_gromacs(installer: &mut Installer) -> Result<(), InstallError> {
    let version = installer.config.gromacs_version.clone();
    let prefix = installer
        .config
        .gromacs_prefix
        .clone()
        .unwrap_or_else(|| installer.tools_root().join("gromacs"));
    let executable = prefix.join("bin/gmx");
    if executable.is_file() {
        let mut version_command = Command::new(&executable);
        version_command.arg("--version");
        if installer.capture(&mut version_command).is_ok_and(|output| {
            String::from_utf8_lossy(&output.stdout).contains(&version)
                || String::from_utf8_lossy(&output.stderr).contains(&version)
        }) {
            installer.note(format!("GROMACS {version} is already installed"));
            return Ok(());
        }
    }

    let scratch = ScratchDir::new_in(installer.tools_root(), "gromacs")?;
    let tarball = format!("gromacs-{version}.tar.gz");
    let archive = scratch.path().join(&tarball);
    installer.download(
        &format!("https://ftp.gromacs.org/gromacs/{tarball}"),
        &archive,
    )?;
    installer.extract_archive(&archive, scratch.path())?;
    let source = scratch.path().join(format!("gromacs-{version}"));
    let build = source.join("build");
    fs::create_dir_all(&build)
        .map_err(|error| InstallError::io("unable to create the GROMACS build directory", error))?;

    let mut configure = Command::new("cmake");
    configure
        .arg("..")
        .arg("-DGMX_BUILD_OWN_FFTW=ON")
        .arg(format!("-DCMAKE_INSTALL_PREFIX={}", prefix.display()))
        .current_dir(&build);
    installer.checked(&mut configure)?;
    let jobs = std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1)
        .to_string();
    let mut build_command = Command::new("cmake");
    build_command
        .args(["--build", ".", "--parallel", &jobs])
        .current_dir(&build);
    installer.checked(&mut build_command)?;
    let mut install_command = Command::new("cmake");
    install_command.args(["--install", "."]).current_dir(&build);
    installer.checked(&mut install_command)?;

    if !executable.is_file() {
        return Err(InstallError::InvalidConfiguration(format!(
            "GROMACS built, but {} was not created",
            executable.display()
        )));
    }
    let mut verify = Command::new(executable);
    verify.arg("--version");
    installer.checked(&mut verify)
}
