use std::{fs, path::Path, process::Command};

use super::{
    InstallError, Installer,
    common::{PipOptions, ScratchDir, TorchBackend},
};
use crate::tool_definitions::Tool;

const ESMFOLD2_ESM_REVISION: &str = "bf343ba264b650dff7a073643725f9aaa1fdbe8d";
const ESMFOLD2_ESM_URL: &str = "https://github.com/Biohub/esm.git";
const ESMFOLD2_STREAMING_LOADER_PATCH: &str =
    include_str!("patches/esmfold2_streaming_loader.patch");

struct FetchedScript {
    name: &'static str,
    url: &'static str,
}

struct UvRecipe<'a> {
    slug: &'static str,
    python: &'static str,
    requirements: &'static [&'static str],
    scripts: &'static [&'static str],
    torch: &'static [&'static str],
    // Installed with ordinary build isolation before `requirements`. Needed when
    // `no_build_isolation` is set and one of `requirements` builds from source against a
    // package -- e.g. setuptools's legacy `setup.py` backend -- that a bare `uv venv` doesn't
    // seed, since with build isolation off there is no isolated build env to put it in instead.
    build_requirements: &'static [&'static str],
    extra_indexes: &'static [&'static str],
    index_strategy: Option<&'static str>,
    no_build_isolation: bool,
    // Whether `requirements` is installed with `--upgrade`. On by default, so that reinstalling a
    // tool moves it to the newest versions its constraints allow. Turn it off where an earlier
    // step of the same recipe installed something the later one must not move -- an exact torch a
    // compiled extension is built against, say, whose version floor in some other requirement
    // would otherwise be met by upgrading it.
    upgrade: bool,
    // Not `'static`: a few recipes (e.g. ESMFold2) need to pass a path computed at install time,
    // such as a scoped CUDA toolchain's prefix, rather than a string literal.
    extra_env: &'a [(&'a str, &'a str)],
    fetched_scripts: &'static [FetchedScript],
    gpu_probe: Option<&'static str>,
    verify: Option<(&'static str, &'static [&'static str])>,
}

impl<'a> UvRecipe<'a> {
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
            build_requirements: &[],
            extra_indexes: &[],
            index_strategy: None,
            no_build_isolation: false,
            upgrade: true,
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
        Tool::Protenix => install_protenix(installer),
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
                    // Sapiens uses the Transformers 4-era Roberta API. Avoid a new major
                    // with different model-loading behavior.
                    "transformers<5",
                    // BioPhi's setup.py reads install_requires from the `pip:` half of
                    // environment.yml, so its conda-side dependencies are absent from a pip
                    // install and have to be named here. abnumber 0.4 can number with ANARCII
                    // and no HMMER, but only when the caller passes use_anarcii=True; BioPhi
                    // calls plain `Chain(...)`, so ANARCI 1.0 and HMMER are what actually run.
                    // Both are on PyPI: `anarci` ships the pressed HMM databases and `hmmer`
                    // ships HMMER 3.4's binaries, so neither needs conda.
                    "abnumber",
                    "anarci",
                    "hmmer",
                ],
                &["biophi"],
            ),
        ),
        Tool::ProteinMpnnDdg => install_proteinmpnn_ddg(installer),
        Tool::RfDiffusion3 => install_rfd3(installer),
        Tool::RfAntibody => install_rfantibody(installer),
        Tool::IgDesign => install_igdesign(installer),
        Tool::ThermoMpnn => install_thermompnn(installer),
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

fn install_recipe(installer: &mut Installer, recipe: UvRecipe<'_>) -> Result<(), InstallError> {
    let backend = (!recipe.torch.is_empty())
        .then(|| installer.select_torch_backend())
        .transpose()?;
    installer.create_venv(recipe.slug, recipe.python)?;
    if let Some(backend) = backend {
        installer.install_torch(recipe.slug, recipe.torch, backend)?;
    }
    installer.pip_install(
        recipe.slug,
        recipe.build_requirements,
        PipOptions::default(),
    )?;
    installer.pip_install(
        recipe.slug,
        recipe.requirements,
        PipOptions {
            extra_indexes: recipe.extra_indexes,
            index_strategy: recipe.index_strategy,
            no_build_isolation: recipe.no_build_isolation,
            extra_env: recipe.extra_env,
            upgrade: recipe.upgrade,
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
    recipe: UvRecipe<'_>,
    url: &str,
    directory: &str,
) -> Result<(), InstallError> {
    install_recipe(installer, recipe)?;
    let target = installer.tools_root().join(directory);
    installer.clone_or_update(url, &target)
}

fn install_thermompnn(installer: &mut Installer) -> Result<(), InstallError> {
    install_checkout_recipe(
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
                    // Imported unconditionally by train_thermompnn.py, including inference.
                    "wandb",
                ],
                &[],
            )
        },
        "https://github.com/Kuhlman-Lab/ThermoMPNN",
        "ThermoMPNN",
    )?;

    // Upstream ships local.yaml with the authors' private /proj/kuhl_lab checkout path. The
    // inference loader uses this setting to find the bundled ProteinMPNN weights, despite the
    // README saying no local.yaml edits are needed for custom inference.
    let checkout = installer.tools_root().join("ThermoMPNN");
    let config_path = checkout.join("local.yaml");
    let source = fs::read_to_string(&config_path).map_err(|error| {
        InstallError::io(format!("unable to read {}", config_path.display()), error)
    })?;
    let escaped_checkout = checkout
        .to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    let mut found = false;
    let mut configured = source
        .lines()
        .map(|line| {
            if line.trim_start().starts_with("thermompnn_dir:") {
                found = true;
                let indentation = &line[..line.len() - line.trim_start().len()];
                format!("{indentation}thermompnn_dir: \"{escaped_checkout}\"")
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    if !found {
        return Err(InstallError::InvalidConfiguration(format!(
            "{} no longer defines platform.thermompnn_dir",
            config_path.display()
        )));
    }
    if source.ends_with('\n') {
        configured.push('\n');
    }
    fs::write(&config_path, configured).map_err(|error| {
        InstallError::io(
            format!("unable to configure {}", config_path.display()),
            error,
        )
    })?;
    Ok(())
}

fn install_esmfold(installer: &mut Installer) -> Result<(), InstallError> {
    // The audited all-atom API is version 3.4.1 in the Biohub repository, while PyPI currently
    // stops at 3.4.0. Keep a patched checkout of the immutable source revision so ESMC-6B's fp32
    // checkpoint tensors are moved and narrowed one at a time instead of filling host RAM before
    // the model is converted to bf16 and transferred to CUDA.
    let source = installer.tools_root().join("esmfold2-esm");
    installer.clone_or_update(ESMFOLD2_ESM_URL, &source)?;

    let mut fetch = Command::new("git");
    fetch.args(["-C"]).arg(&source).args([
        "fetch",
        "--depth",
        "1",
        "origin",
        ESMFOLD2_ESM_REVISION,
    ]);
    installer.checked(&mut fetch)?;

    let mut checkout = Command::new("git");
    checkout
        .args(["-C"])
        .arg(&source)
        .args(["checkout", "--detach", "FETCH_HEAD"]);
    installer.checked(&mut checkout)?;

    let scratch = ScratchDir::new_in(installer.tools_root(), "esmfold2-patch")?;
    let patch = scratch.path().join("streaming-loader.patch");
    fs::write(&patch, ESMFOLD2_STREAMING_LOADER_PATCH)
        .map_err(|error| InstallError::io(format!("unable to write {}", patch.display()), error))?;
    let mut check_patch = Command::new("git");
    check_patch
        .args(["-C"])
        .arg(&source)
        .args(["apply", "--check"])
        .arg(&patch);
    installer.checked(&mut check_patch)?;
    let mut apply_patch = Command::new("git");
    apply_patch
        .args(["-C"])
        .arg(&source)
        .arg("apply")
        .arg(&patch);
    installer.checked(&mut apply_patch)?;

    let requirement = source.to_string_lossy().into_owned();
    installer.create_venv(Tool::EsmFold2.slug(), "3.12")?;
    installer.pip_install(
        Tool::EsmFold2.slug(),
        &[&requirement],
        PipOptions::default(),
    )?;
    let mut verify = Command::new(installer.venv_python(Tool::EsmFold2.slug()));
    verify.args([
        "-c",
        "from inspect import signature; from esm.models.esmfold2 import ESMFold2InputBuilder, EsmFold2Model; from esm.models.hub import read_safetensors_dir; assert 'dtype' in signature(read_safetensors_dir).parameters",
    ]);
    installer.checked(&mut verify)
}

fn install_protenix(installer: &mut Installer) -> Result<(), InstallError> {
    install_recipe(
        installer,
        UvRecipe {
            gpu_probe: Some(
                "import torch; assert torch.cuda.is_available(), 'Protenix requires CUDA'",
            ),
            ..UvRecipe::simple(Tool::Protenix.slug(), "3.11", &["protenix"], &["protenix"])
        },
    )?;

    // Protenix JIT-compiles a fused layer-norm CUDA extension the first time
    // `protenix.model.layer_norm` is imported -- which happens on every CLI invocation, not just
    // at install time -- via `torch.utils.cpp_extension.load()`. That build shells out to
    // whatever `nvcc`/`gcc` it finds, and this machine's system compiler is newer than what
    // torch==2.7.1's bundled ATen headers compile under, so every later invocation used to fail.
    // Building it once here with a scoped, torch-compatible toolchain seeds
    // torch's on-disk extension cache (keyed by source hash, outside this venv), so ordinary
    // runtime invocations -- which still use the system compiler -- find a cached build and never
    // need to invoke a compiler at all.
    let prefix = installer.ensure_cuda_toolchain(
        Tool::Protenix.slug(),
        "12.6",
        &[
            "libcusparse-dev",
            "libcublas-dev",
            "libcusolver-dev",
            "libcurand-dev",
            "libcufft-dev",
            "gcc_linux-64=13",
            "gxx_linux-64=13",
        ],
    )?;
    let cuda_home = prefix.to_string_lossy().into_owned();
    let cc = prefix
        .join("bin")
        .join("x86_64-conda-linux-gnu-gcc")
        .to_string_lossy()
        .into_owned();
    let cxx = prefix
        .join("bin")
        .join("x86_64-conda-linux-gnu-g++")
        .to_string_lossy()
        .into_owned();
    let library_path = prefix.join("lib").to_string_lossy().into_owned();
    // conda-forge's `cuda-cudart-dev` follows the NVIDIA redistributable layout and puts
    // `cuda_runtime_api.h` etc. under `targets/x86_64-linux/include`, not `include` directly, so
    // torch's own `-I$CUDA_HOME/include` isn't enough for host (non-nvcc) compilation of files
    // that pull in c10's CUDA headers. `CPATH` is honored by gcc/g++/nvcc alike.
    let cuda_target_include = prefix
        .join("targets")
        .join("x86_64-linux")
        .join("include")
        .to_string_lossy()
        .into_owned();
    // `ninja` (needed by torch's JIT loader) is a console script installed into the venv itself,
    // not on the installer process's own PATH, so it has to be added here explicitly -- the same
    // thing `installer.pip_install`/`tool_command` do for every other in-venv invocation.
    let venv_scripts = installer.venv_scripts_dir(Tool::Protenix.slug());
    let path = std::env::join_paths(
        std::iter::once(venv_scripts).chain(
            std::env::var_os("PATH")
                .map(|path| std::env::split_paths(&path).collect::<Vec<_>>())
                .unwrap_or_default(),
        ),
    )
    .map_err(|error| {
        InstallError::InvalidConfiguration(format!(
            "unable to build PATH for Protenix's warm-up build: {error}"
        ))
    })?;

    let mut warm_up = Command::new(installer.venv_python(Tool::Protenix.slug()));
    warm_up
        .args(["-c", "import protenix.model.layer_norm.layer_norm"])
        .env("NVCC_APPEND_FLAGS", "-std=c++17")
        .env("CUDA_HOME", &cuda_home)
        .env("CC", &cc)
        .env("CXX", &cxx)
        .env("LIBRARY_PATH", &library_path)
        .env("LD_LIBRARY_PATH", &library_path)
        .env("CPATH", &cuda_target_include)
        .env("PATH", &path);
    installer.checked(&mut warm_up)
}

fn install_proteinmpnn_ddg(installer: &mut Installer) -> Result<(), InstallError> {
    let backend = installer.select_torch_backend()?;
    let (requirements, gpu_probe): (&[&str], Option<&str>) = if backend == TorchBackend::Cuda126 {
        (
            &[
                "ProteinMPNN-ddG[cuda12] @ git+https://github.com/PeptoneLtd/proteinmpnn_ddg.git@main",
                "dm-haiku==0.0.13",
            ],
            Some(
                "import jax; assert any(d.platform == 'gpu' for d in jax.devices()), \
                 'ProteinMPNN-ddG CUDA installation does not report a JAX GPU'",
            ),
        )
    } else {
        (
            &[
                "ProteinMPNN-ddG @ git+https://github.com/PeptoneLtd/proteinmpnn_ddg.git@main",
                "dm-haiku==0.0.13",
            ],
            None,
        )
    };
    install_recipe(
        installer,
        UvRecipe {
            fetched_scripts: &[FetchedScript {
                name: "proteinmpnn-ddg",
                url: "https://raw.githubusercontent.com/PeptoneLtd/proteinmpnn_ddg/main/predict.py",
            }],
            gpu_probe,
            ..UvRecipe::simple(
                Tool::ProteinMpnnDdg.slug(),
                "3.10",
                requirements,
                &["proteinmpnn-ddg"],
            )
        },
    )
}

fn install_rfd3(installer: &mut Installer) -> Result<(), InstallError> {
    install_recipe(
        installer,
        UvRecipe {
            torch: &["torch==2.7.1"],
            // rc-foundry floors torch at 2.2 and nothing above that, so `--upgrade` would read
            // that floor as licence to replace the backend-specific wheel installed above with
            // whatever PyPI serves by default -- a CUDA build on a host that may have no GPU.
            upgrade: false,
            gpu_probe: Some(
                "import torch; assert torch.cuda.is_available(), 'RFdiffusion3 requires CUDA'",
            ),
            ..UvRecipe::simple(
                Tool::RfDiffusion3.slug(),
                // rc-foundry publishes for 3.12 alone: `requires-python = ">=3.12,<3.13"`.
                "3.12",
                // The extra adds pydantic, which the input specification is built out of; the
                // model code itself is in the base distribution alongside RF3 and MPNN.
                &["rc-foundry[rfd3]"],
                &["rfd3", "foundry"],
            )
        },
    )?;
    // `foundry install rfd3` fetches exactly this file, but also rewrites the installed package's
    // own .env to record where it went. Download it directly instead, under the name foundry's
    // checkpoint registry knows it by, so nothing inside the environment is edited after the fact
    // and a consumer can point `ckpt_path` straight at it.
    let checkpoints = installer.tools_root().join("rfd3").join("checkpoints");
    installer.download(
        "https://files.ipd.uw.edu/pub/rfd3/rfd3_foundry_2025_12_01_remapped.ckpt",
        &checkpoints.join("rfd3_latest.ckpt"),
    )
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
